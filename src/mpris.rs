use crate::tui::{App, MpvCommand, MpvState};
#[cfg(target_os = "linux")]
use souvlaki::PlatformConfig;
use souvlaki::{MediaControlEvent, MediaControls, MediaPosition, SeekDirection};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::oneshot;
use crate::database::database::{Command, JellyfinCommand};

// linux only, macos requires a window and windows is unsupported
pub fn mpris() -> Result<MediaControls, Box<dyn std::error::Error>> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err("mpris is only supported on linux".into());
    }

    #[cfg(target_os = "linux")]
    {
        let hwnd = None;

        let config = PlatformConfig {
            dbus_name: "jellyfin-tui",
            display_name: "jellyfin-tui",
            hwnd,
        };

        Ok(MediaControls::new(config).unwrap())
    }
}
pub fn register_controls(
    controls: &mut MediaControls,
    mpris_tx: std::sync::mpsc::Sender<MediaControlEvent>,
) {
    if let Err(e) = controls.attach(move |event| {
        // never block, never lock
        let _ = mpris_tx.send(event);
    }) {
        log::error!("Failed to attach media controls: {:#?}", e);
    }
}

impl App {
    /// Registers the media controls to the MpvState. Called after each mpv thread re-init.

    pub fn update_mpris_position(&mut self, secs: f64) -> Option<()> {
        let progress = MediaPosition(
            Duration::try_from_secs_f64(secs).unwrap_or(Duration::ZERO)
        );

        let controls = self.controls.as_mut()?;

        controls
            .set_playback(if self.paused {
                souvlaki::MediaPlayback::Paused { progress: Some(progress) }
            } else {
                souvlaki::MediaPlayback::Playing { progress: Some(progress) }
            })
            .ok()?;

        Some(())
    }

    pub async fn handle_mpris_events(&mut self) {
        while let Ok(event) = self.mpris_rx.try_recv() {
            match event {
                MediaControlEvent::Toggle => {
                    if let Err(e) = match self.paused {
                        true => self.play().await,
                        false => self.pause().await
                    } {
                        log::error!("Error in pause action: {}", e);
                    }
                }
                MediaControlEvent::Next => {
                    if let Err(e) = self.next().await {
                        log::error!("Error in next action{}", e);
                    }
                }
                MediaControlEvent::Previous => {
                    if let Err(e) = self.previous().await {
                        log::error!("Error in previosu action{}", e);
                    }
                }
                MediaControlEvent::Stop => {
                    if let Err(e) = self.stop().await {
                        log::error!("Error in stop action{}", e);
                    }
                }
                MediaControlEvent::Play => {
                    if let Err(e) = self.play().await {
                        log::error!("MPRIS: Error in play action: {}", e);
                    }
                }
                MediaControlEvent::Pause => {
                    if let Err(e) = self.pause().await {
                        log::error!("MPRIS: Error in play action: {}", e);
                    }
                }
                MediaControlEvent::SeekBy(direction, duration) => {
                    let rel = duration.as_secs_f64()
                        * (if matches!(direction, SeekDirection::Forward) {
                            1.0
                        } else {
                            -1.0
                        });

                    self.update_mpris_position(self.state.current_playback_state.position + rel);

                    if let Err(e) = self
                        .mpv_send(|reply| MpvCommand::Seek {
                            amount: *rel,
                            relative: true,
                            reply,
                        }).await {
                        log::error!("MPRIS: Error seeking: {}", e);
                    }
                }
                MediaControlEvent::SetPosition(position) => {
                    let secs = position.0.as_secs_f64();
                    self.update_mpris_position(secs);

                    let _ = mpv.mpv.command("seek", &[&secs.to_string(), "absolute"]);
                }
                MediaControlEvent::SetVolume(_volume) => {
                    #[cfg(target_os = "linux")]
                    {
                        let volume = _volume.clamp(0.0, 1.5);
                        self.mpv_send(|reply| MpvCommand::SetVolume {
                            volume,
                            reply,
                        }).await?;
                        self.state.current_playback_state.volume = (volume * 100.0) as i64;
                        if let Some(ref mut controls) = self.controls {
                            let _ = controls.set_volume(volume);
                        }
                    }
                }
                _ => {}
            }
        }
        mpv.mpris_events.clear();
    }
}

use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use libmpv2::{Format, Mpv};
use souvlaki::MediaControlEvent;
use tokio::sync::oneshot;
use tokio::time::Instant;
use crate::database::database::{Command, JellyfinCommand};
use crate::helpers;
use crate::tui::{App, MpvPlaybackState, Repeat, Song};

#[derive(Clone)]
pub struct MpvHandle {
    tx: Sender<MpvCommand>,
}


type MpvResult = Result<(), libmpv2::Error>;
type Reply<T = ()> = oneshot::Sender<MpvResult>;

pub enum MpvCommand {
    Play { reply: Reply },
    Pause { reply: Reply },
    Stop { reply: Reply },
    Seek { amount: f64, relative: bool, reply: Reply },
    Next { reply: Reply },
    Previous { current_time: f64, reply: Reply },
    SetVolume { volume: f64, reply: Reply },
    SetRepeat { repeat: Repeat, reply: Reply },
    LoadState { state: MpvPlaybackState, reply: Reply },
    LoadQueue {
        songs: Vec<Song>,
        start_index: i64,
        volume: i64,
        repeat: Repeat,
        reply: Reply,
    },
    LoadFile { url: String, action: & 'static str, position: Option<usize>, reply: Reply },
    PlayIndex { index: i64, reply: Reply },
    PlaylistRemove { index: usize, reply: Reply },
    PlaylistMove { from: usize, to: usize, reply: Reply },
}

impl App {

    pub fn init_mpv(config: &serde_yaml::Value) -> libmpv2::Mpv {
        let mpv = Mpv::with_initializer(|mpv| {
            mpv.set_option("msg-level", "ffmpeg/demuxer=no").unwrap();
            Ok(())
        })
            .expect(" [XX] Failed to initiate mpv context");
        mpv.set_property("vo", "null").unwrap();
        mpv.set_property("volume", 100).unwrap();
        mpv.set_property("prefetch-playlist", "yes").unwrap(); // gapless playback

        // no console output (it shifts the tui around)
        let _ = mpv.set_property("quiet", "yes");
        let _ = mpv.set_property("really-quiet", "yes");

        // optional mpv options (hah...)
        if let Some(mpv_config) = config.get("mpv") {
            if let Some(mpv_config) = mpv_config.as_mapping() {
                for (key, value) in mpv_config {
                    if let (Some(key), Some(value)) = (key.as_str(), value.as_str()) {
                        mpv.set_property(key, value).unwrap_or_else(|e| {
                            panic!("This is not a valid mpv property {key}: {:?}", e)
                        });
                        log::info!("Set mpv property: {} = {}", key, value);
                    }
                }
            } else {
                log::error!("mpv config is not a mapping");
            }
        }

        mpv.disable_deprecated_events().unwrap();
        mpv.observe_property("volume", Format::Int64, 0).unwrap();
        mpv.observe_property("demuxer-cache-state", Format::Node, 0)
            .unwrap();

        // self.mpv_thread = Some(thread::spawn(move || {
        //     if let Err(e) = Self::t_playlist(songs, mpv_state, sender, state, repeat) {
        //         log::error!("Error in mpv playlist thread: {}", e);
        //     }
        // }));
        return mpv;
    }

    // pub async fn mpv_send(
    //     &self,
    //     cmd: impl AsyncFnOnce(
    //         oneshot::Sender<Result<(), Box<dyn std::error::Error + Send>>>
    //     ) -> MpvCommand,
    // ) -> Result<(), Box<dyn std::error::Error>> {
    //     let (tx, rx) = oneshot::channel();
    //
    //     self.mpv_cmd_tx
    //         .send(cmd(tx))
    //         .map_err(|_| "mpv thread is not running".into())?;
    //
    //     rx.await.map_err(|_| "mpv thread died mid-command".into())?
    // }
    //
    //
    //
    // pub async fn mpv_send(
    //     &self,
    //     cmd: impl FnOnce(Reply) -> MpvCommand,
    // ) -> Result<(), std::error::Error> {
    //     let (tx, rx) = oneshot::channel();
    //
    //     self.mpv_cmd_tx
    //         .send(cmd(tx))
    //         .await
    //         .map_err(|_| "mpv thread is not running".to_string())?;
    //
    //
    //     rx.await
    //         .map_err(|_| "mpv thread died".to_string())?
    // }
    //
    //
    pub async fn mpv_send(
        &self,
        cmd: impl FnOnce(oneshot::Sender<Result<(), libmpv2::Error>>) -> MpvCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = oneshot::channel();

        self.mpv_cmd_tx
            .send(cmd(tx))
            .await
            .map_err(|_| "mpv thread not running")?;

        let result = rx.await.map_err(|_| "mpv thread died")?;
        result?; // libmpv::Error auto-boxes

        Ok(())
    }


    pub async fn mpv_start_playlist(
        &mut self,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // let sender = self.sender.clone();
        // let songs = self.state.queue.clone();

        // if self.mpv_thread.is_some() {
        //     if let Ok(mpv) = self.mpv_state.lock() {
        //         let _ = mpv.mpv.command("stop", &[]);
        //         for song in &songs {
        //             match helpers::normalize_mpvsafe_url(&song.url) {
        //                 Ok(safe_url) => {
        //                     let _ = mpv
        //                         .mpv
        //                         .command("loadfile", &[safe_url.as_str(), "append-play"]);
        //                 }
        //                 Err(e) => {
        //                     log::error!("Failed to normalize URL '{}': {:?}", song.url, e);
        //                     if e.to_string().contains("No such file or directory") {
        //                         let _ = self
        //                             .db
        //                             .cmd_tx
        //                             .send(Command::Update(UpdateCommand::OfflineRepair))
        //                             .await;
        //                     }
        //                 }
        //             }
        //         }
        //         let _ = mpv.mpv.set_property("pause", false);
        //         self.paused = false;
        //         self.song_changed = true;
        //     }
        //     return Ok(());
        // }

        // if let Some(ref mut controls) = self.controls {
        //     if controls.detach().is_ok() {
        //         App::register_controls(controls, &self.mpv_cmd_tx);
        //     }
        // }

        // let cmd = MpvCommand::LoadQueue {
        //     songs: self.state.queue.clone(),
        //     start_index: self.state.current_playback_state.current_index,
        //     volume: self.state.current_playback_state.volume,
        //     repeat: self.preferences.repeat.clone(),
        // };
        // self.mpv_cmd_tx
        //     .as_ref()
        //     .unwrap()
        //     .send(cmd)?;
        //
        self.mpv_send(|reply| MpvCommand::LoadQueue {
            songs: self.state.queue.clone(),
            start_index: self.state.current_playback_state.current_index,
            volume: self.state.current_playback_state.volume,
            repeat: self.preferences.repeat.clone(),
            reply,
        }).await?;
        self.paused = false;

        Ok(())
    }


    /// The thread that keeps in sync with the mpv thread
    pub fn mpv_runtime(
        mut mpv_cmd_rx: tokio::sync::mpsc::Receiver<MpvCommand>,
        mpv: libmpv2::Mpv,
        sender: Sender<MpvPlaybackState>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {

        let _ = mpv.command("playlist_clear", &["force"]);

        let mut last = MpvPlaybackState {
            position: 0.0,
            duration: 0.0,
            current_index: 0,
            last_index: 0,
            volume: 100,
            audio_bitrate: 0,
            audio_samplerate: 0,
            hr_channels: String::new(),
            file_format: String::new(),
        };
        let mut last_update = Instant::now();

        loop {
            while let Ok(cmd) = mpv_cmd_rx.try_recv() {
                match cmd {
                    MpvCommand::Play { reply } => {
                        let res = mpv.set_property("pause", false);
                        let _ = reply.send(res);
                    }
                    MpvCommand::Pause { reply } => {
                        let res = mpv.set_property("pause", true);
                        let _ = reply.send(res);
                    }
                    MpvCommand::Seek { relative, amount, reply } => {
                        if relative {
                            let res = mpv.command("seek", &[&amount.to_string()]);
                            let _ = reply.send(res);
                        } else {
                            let res = mpv.command("seek", &[&amount.to_string(), "absolute"]);
                            let _ = reply.send(res);
                        }
                    }
                    MpvCommand::Next { reply } => {
                        let res = mpv.command("playlist-next", &[]);
                        let _ = reply.send(res);
                    }
                    MpvCommand::Previous { current_time , reply } => {
                        let res = if current_time > 5.0 {
                            mpv.command("seek", &["0.0", "absolute"])
                        } else {
                            mpv.command("playlist-prev", &["force"])
                        };
                        let _ = reply.send(res);
                    }
                    MpvCommand::SetVolume { volume, reply } => {
                        let res = mpv.set_property("volume", (volume * 100.0) as i64);
                        let _ = reply.send(res);
                    }
                    MpvCommand::SetRepeat { repeat, reply } => {
                        let res = match repeat {
                            Repeat::None => {
                                let _ = mpv.set_property("loop-file", "no");
                                mpv.set_property("loop-playlist", "no")
                            }
                            Repeat::All => mpv.set_property("loop-playlist", "inf"),
                            Repeat::One => {
                                let _ = mpv.set_property("loop-playlist", "no");
                                mpv.set_property("loop-file", "inf")
                            }
                        };
                        let _ = reply.send(res);
                    }
                    MpvCommand::LoadState { state, reply } => {
                        let _ = mpv.set_property("volume", state.volume);
                        // let _ = mpv.set_property("playlist-pos", state.current_index);
                        let _ = reply.send(Ok(()));
                    }
                    MpvCommand::LoadQueue { songs, start_index, volume, repeat, reply } => {
                        let _ = mpv.command("playlist_clear", &["force"]);
                        for song in songs {
                            match helpers::normalize_mpvsafe_url(&song.url) {
                                Ok(safe_url) => {
                                    let _ = mpv
                                        .command("loadfile", &[safe_url.as_str(), "append-play"]);
                                }
                                Err(e) => log::error!("Failed to normalize URL '{}': {:?}", song.url, e),
                            }
                        }
                        let _ = mpv.set_property("volume", volume);
                        let _ = mpv.set_property("playlist-pos", start_index);

                        match repeat {
                            Repeat::None => {
                                let _ = mpv.set_property("loop-file", "no");
                                let _ = mpv.set_property("loop-playlist", "no");
                            }
                            Repeat::All => {
                                let _ = mpv.set_property("loop-playlist", "inf");
                            }
                            Repeat::One => {
                                let _ = mpv.set_property("loop-playlist", "no");
                                let _ = mpv.set_property("loop-file", "inf");
                            }
                        }

                        let _ = mpv.set_property("pause", false);
                        let _ = reply.send(Ok(()));
                    },
                    MpvCommand::PlayIndex { index, reply } => {
                        let res = mpv.set_property("playlist-pos", index);
                        let _ = reply.send(res);
                    }
                    MpvCommand::LoadFile { url, action, position, reply } => {
                        match helpers::normalize_mpvsafe_url(&url) {
                            Ok(safe_url) => {
                                let res = if let Some(pos) = position {
                                    mpv.command("loadfile", &[safe_url.as_str(), action, &pos.to_string()])
                                } else {
                                    mpv.command("loadfile", &[safe_url.as_str(), action])
                                };
                                let _ = reply.send(res);
                            }
                            Err(e) => {
                                log::error!("Failed to normalize URL '{}': {:?}", url, e);
                                let res = Err(libmpv2::Error::Null);
                                let _ = reply.send(res);
                            }
                        }
                    }
                    MpvCommand::PlaylistRemove { index, reply } => {
                        let res = mpv
                            .command("playlist-remove", &[&index.to_string()]);
                        let _ = reply.send(res);
                    }
                    MpvCommand::PlaylistMove { from, to, reply } => {
                        let res = mpv
                            .command("playlist-move", &[&from.to_string(), &to.to_string()]);
                        let _ = reply.send(res);
                    }
                    _ => {} // TODO: remove
                }
            }

            let position = mpv.get_property("time-pos").unwrap_or(0.0);
            let current_index: i64 = mpv.get_property("playlist-pos").unwrap_or(0);
            let duration = mpv.get_property("duration").unwrap_or(0.0);
            let volume = mpv.get_property("volume").unwrap_or(0);
            let audio_bitrate = mpv.get_property("audio-bitrate").unwrap_or(0);
            let audio_samplerate = mpv.get_property("audio-params/samplerate").unwrap_or(0);
            // let audio_channels = mpv.mpv.get_property("audio-params/channel-count").unwrap_or(0);
            // let audio_format: String = mpv.mpv.get_property("audio-params/format").unwrap_or_default();
            let hr_channels: String = mpv
                .get_property("audio-params/hr-channels")
                .unwrap_or_default();

            let file_format: String = mpv.get_property("file-format").unwrap_or_default();

            if (position - last.position).abs() < 0.95
                && (duration - last.duration).abs() < 0.95
                && current_index == last.current_index
                && volume == last.volume
                && last_update.elapsed() < Duration::from_millis(750)
            {
                thread::sleep(Duration::from_secs_f32(0.2));
                continue;
            }
            last_update = Instant::now();

            last = MpvPlaybackState {
                position,
                duration,
                current_index,
                last_index: last.last_index,
                volume,
                audio_bitrate,
                audio_samplerate,
                hr_channels,
                file_format: file_format.to_string(),
            };

            let _ = sender.send(last.clone());

            thread::sleep(Duration::from_secs_f32(0.2));
        }
    }


    pub async fn play(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.paused {
            return Ok(());
        }
        self.mpv_send(|reply| MpvCommand::Play {
            reply,
        }).await?;
        self.paused = false;

        let _ = self.handle_discord(true).await;
        let current_song = self.state.queue
            .get(self.state.current_playback_state.current_index as usize)
            .cloned()
            .unwrap_or_default();
        let _ = self.report_progress_if_needed(&current_song, true).await;

        self.update_mpris_position(self.state.current_playback_state.position);
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.paused {
            return Ok(());
        }
        self.mpv_send(|reply| MpvCommand::Pause {
            reply,
        }).await?;
        self.paused = true;

        let _ = self.handle_discord(true).await;
        let current_song = self.state.queue
            .get(self.state.current_playback_state.current_index as usize)
            .cloned()
            .unwrap_or_default();
        let _ = self.report_progress_if_needed(&current_song, true).await;

        self.update_mpris_position(self.state.current_playback_state.position);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.mpv_send(|reply| MpvCommand::Stop {
            reply,
        }).await?;
        self.state.queue.clear();
        self.lyrics = None;
        self.cover_art = None;
        if let Some(controls) = self.controls.as_mut() {
            let _ = controls.set_playback(souvlaki::MediaPlayback::Stopped);
        }
        Ok(())
    }

    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.client.is_some() {
            let _ = self
                .db
                .cmd_tx
                .send(Command::Jellyfin(JellyfinCommand::Stopped {
                    id: Some(self.active_song_id.clone()),
                    position_ticks: Some(self.state.current_playback_state.position as u64
                        * 10_000_000
                    ),
                }))
                .await;
        }
        self.mpv_send(|reply| MpvCommand::Next {
            reply,
        }).await?;
        self.update_mpris_position(0.0);
        Ok(())
    }

    pub async fn previous(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.mpv_send(|reply| MpvCommand::Previous {
            current_time: self.state.current_playback_state.position,
            reply,
        }).await?;
        self.update_mpris_position(0.0);
        Ok(())
    }
}
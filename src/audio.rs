use std::fs;
use std::path::PathBuf;
 
use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use yt_dlp::model::Video;
use yt_dlp::Downloader;

use crate::constants::OUTPUT_FILE_PATH;

struct Track {
    video: Video,
    path: PathBuf,
}

pub struct Audio {
    downloader: Downloader,

    // current content
    current: Option<Track>,

    // loaded content (gets swapped in next)
    loaded: Option<Track>,

    handle: MixerDeviceSink,
    player: Player
}

impl Audio {
    pub async fn new() -> Result<Self> {
        let libs_dir = PathBuf::from("libs");
        let output_dir = PathBuf::from(OUTPUT_FILE_PATH);

        let downloader = Downloader::with_new_binaries(libs_dir.clone(), output_dir.clone())
            .await
            .context("failed to initialize yt-dlp binaries")?
            .build()
            .await
            .context("failed to initialize yt-dlp downloader")?;

        let handle = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(&handle.mixer());

        Ok(Self {
            downloader,
            
            current: None,
            loaded: None,

            handle,
            player
        })    
    }   

    pub async fn load(&mut self, url: &str) -> Result<()> {
        println!("Loading ...");
        let video = self.downloader.fetch_video_infos(url.to_string())
            .await.context("failed to fetch video metadata")?;
        
        let filename = format!("{}.m4a", Self::sanitize_filename(&video.title));

        let path = self.downloader.download_audio_stream(&video, &filename)
            .await.context("failed to download audio stream")?;

        if self.current.is_some() && self.loaded.is_some() {
            let loaded_track = self.loaded.take().unwrap();
            let current_track = self.current.take().unwrap();

            // if loaded and current are the same, then loaded
            // has been loaded intlo current, so it will be deleted.
            if loaded_track.path != current_track.path {
                // If something was loaded, but wasn't ever played, delete it
                let _ = self.delete_file(loaded_track);
            }
        }

        println!("Loaded track: {}", video.title);

        self.loaded = Some(Track {
            video,
            path,
        });



        Ok(())
    }

    pub fn swap(&mut self) -> Result<()> {
        // a few steps here

        // 1. delete the current video and its artifacts
        // 2. swap in the loaded video into the current

        // delete the current video and artifacts
        
        println!("Started audio swap");


        let next = self.loaded.take().context("no track preloaded")?;

        self.discard_current_playback()?;

        let file = std::fs::File::open(&next.path)?;
        let decoder = Decoder::try_from(file)?;

        self.player.stop();
        self.player.append(decoder);
        self.player.pause();

        self.current = Some(next);

        println!("Swapped in track: {}", self.current.as_ref().unwrap().video.title);

        Ok(())
    }

    fn discard_current_playback(&mut self) -> Result<()> {
        self.player.stop();
        if self.current.is_some(){
            let current_track = self.current.take().unwrap();
            let _ = self.delete_file(current_track);
        }

        Ok(())
    }

    fn delete_file(&mut self, track: Track) -> Result<()> {
        // delete the current playing file

        if track.path.exists() {
            fs::remove_file(&track.path).with_context(|| format!("failed to delete file at {}", track.path.display()))?;
        }
        
        Ok(())
    }

    /// Pause the current track.
    pub fn pause(&self) {
        self.player.pause();
        println!("Paused track");
    }

    /// Play a track.
    pub fn play(&self) {
        self.player.play();
        println!("Played track");
    }

    fn sanitize_filename(title: &str) -> String {
        title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
    }
}
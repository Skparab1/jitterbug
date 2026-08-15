use std::fs;
use std::path::PathBuf;
use std::collections::VecDeque;
 
use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use yt_dlp::model::Video;
use yt_dlp::model::selector::{AudioQuality, AudioCodecPreference};
use yt_dlp::Downloader;

use crate::constants::{packet_types, OUTPUT_FILE_PATH};

// Time syncing
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep_until, Duration, Instant};

struct Track {
    title: String,
    duration: Option<i64>, // seconds
    video: Video,
    path: PathBuf,
}

pub struct Audio {
    downloader: Downloader,

    // current content
    current: Option<Track>,

    queue: VecDeque<Track>,

    handle: MixerDeviceSink,
    player: Player
}

impl Audio {
    pub async fn new() -> Result<Self> {
        let libs_dir = PathBuf::from("libs");
        let output_dir = PathBuf::from(OUTPUT_FILE_PATH);


        let downloader = Downloader::with_new_binaries(libs_dir.clone(), output_dir.clone())
        .await?
        .with_cookies_from_browser("chrome") // or "firefox", "safari", etc.
        .build()
        .await?;
        
        
        downloader.update_downloader().await?;

        let handle = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(&handle.mixer());

        let queue = VecDeque::new();

        Ok(Self {
            downloader,
            
            current: None,
            queue,

            handle,
            player
        })    
    }   

    pub fn get_pos(&self) -> Duration {
        self.player.get_pos()
    }

    pub fn preflight_check(&mut self, action_type: u8) -> Result<bool>{
        if action_type == packet_types::AUDIO_SWAP {
            return Ok(self.queue.front().is_some());
        } else if action_type == packet_types::AUDIO_PAUSE || action_type == packet_types::AUDIO_PLAY ||
                  action_type == packet_types::AUDIO_FWD || action_type == packet_types::AUDIO_BACK {
            return Ok(self.current.is_some());
            // for now, no checks if the audio is already playing and a "play" signal is sent
        }
        
        Ok(true)
    }

    pub async fn load(&mut self, url: &str) -> Result<()> {
        println!("Loading ...");
        let video = self.downloader.fetch_video_infos(url.to_string())
            .await.context("failed to fetch video metadata")?;
        
        let filename = format!("{}.mp3", Self::sanitize_filename(&video.title));

        // let path = self.downloader.download_audio_stream(&video, &filename)
        //     .await.context("failed to download audio stream")?;

        let path = self.downloader
            .download_audio_stream_with_quality(
                &video,
                &filename,
                AudioQuality::High,
                AudioCodecPreference::AAC,
            )
            .await
            .context("failed to download audio stream")?;

        println!("Loaded track: {}", video.title);

        self.queue.push_back(Track {
            title: video.title.clone(),
            duration: video.duration,
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
        
        self.discard_current_playback()?;

        let next = self.queue.pop_front().context("no loaded track to swap in")?;

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

    // Pause the current track
    pub fn pause(&self) {
        self.player.pause();
        println!("Paused track");
    }

    pub async fn play_at(&self, to_set_timestamp: u128, when_to_play_timestamp: u128){

        println!("PlayAt invoked");
        
        self.player.try_seek(Duration::from_secs((to_set_timestamp / 1000) as u64));

        println!("Seeked to: {} ms", to_set_timestamp);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        println!("Now will wait until: {} ms", when_to_play_timestamp);

        if when_to_play_timestamp > now_ms {
            let ms_to_wait = (when_to_play_timestamp - now_ms) as u64;
            
            // Map millisecond duration to Tokio's monotonic clock
            let deadline = Instant::now() + Duration::from_millis(ms_to_wait);
            sleep_until(deadline).await;
        }

        println!("Wait time elapsed. Now playing...");

        self.play();

    }

    // Play a track
    pub fn play(&self) {
        self.player.play();
        println!("Played track");
    }

    pub fn forward(&self){
        let current_pos = self.player.get_pos();

        let _ = self.player.try_seek(current_pos + Duration::from_secs(5));
    }

    pub fn backward(&self){
        let current_pos = self.player.get_pos();
        // this is so that it never gets negative
        let new_pos = current_pos.saturating_sub(Duration::from_secs(5));

        let _ = self.player.try_seek(new_pos);
    }

    pub fn set_volume(&mut self, volume: u128) {
        let volume = (volume as f32) / 100.0; // Convert to a value between 0.0 and 1.0
        self.player.set_volume(volume);
        println!("Volume set to: {}", volume);
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
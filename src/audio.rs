use std::fs;
use std::path::PathBuf;
use std::collections::VecDeque;

use anyhow::{Context, Result};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use yt_dlp::Downloader;

use crate::constants::{packet_types};

// Time syncing
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep_until, Duration, Instant};
use tokio::process::Command;

struct Track {
    title: String,
    duration: Option<i64>, // seconds
    path: PathBuf,
}

pub struct Audio {
    downloader: Downloader,

    // current content
    current: Option<Track>,

    queue: VecDeque<Track>,

    handle: MixerDeviceSink,
    player: Player,
    output_file_path: String,
}

impl Audio {
    pub async fn new(output_file_path: String) -> Result<Self> {
        let libs_dir = PathBuf::from("libs");
        let output_dir = PathBuf::from(&output_file_path);


        let downloader = Downloader::with_new_binaries(libs_dir.clone(), output_dir.clone())
        .await?
        .with_cookies_from_browser("chrome") // or "firefox", "safari", etc.
        .build()
        .await?;
        

        let handle = DeviceSinkBuilder::open_default_sink()?;
        let player = Player::connect_new(&handle.mixer());

        let queue = VecDeque::new();

        Ok(Self {
            downloader,
            
            current: None,
            queue,

            handle,
            player,
            
            output_file_path,
        })    
    }   

    pub fn get_pos(&self) -> Duration {
        self.player.get_pos()
    }

    pub fn get_queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn get_current_track_title(&self) -> Option<String> {
        self.current.as_ref().map(|track| track.title.clone())
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
        
        let _video_id = &video.id;
        let title = video.title.clone();
        let duration = video.duration;

        let output_dir = PathBuf::from(self.output_file_path.clone());
        fs::create_dir_all(&output_dir)?;

        let output_template = output_dir.join("%(id)s.%(ext)s");

        // we run this as if it is a shell command 
        let output = Command::new("yt-dlp")
            .arg("-x")
            .arg("--audio-format")
            .arg("mp3")
            .arg("--audio-quality")
            .arg("0")
            .arg("--cookies-from-browser")
            .arg("firefox")
            .arg("-o")
            .arg(output_template.to_string_lossy().to_string())
            .arg(url)
            .output()
            .await
            .context("failed to execute yt-dlp subprocess")?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp download failed: {}", error_msg);
        }

        let final_path = fs::read_dir(&output_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(&video.id) && s.ends_with(".mp3"))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("Download succeeded, but no MP3 file was found for video ID {}", video.id))?;

        self.queue.push_back(Track {
            title,
            duration,
            path: final_path,
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

        println!("Swapped in track: {}", self.current.as_ref().unwrap().title);

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
        
        let _ = self.player.try_seek(Duration::from_secs((to_set_timestamp / 1000) as u64));

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
}
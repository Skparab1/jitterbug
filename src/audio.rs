use std::fs;
use std::path::PathBuf;
 
use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
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

    stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Sink
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

        let (stream, stream_handle) =
                OutputStream::try_default().context("failed to open default audio output device")?;
        let sink = Sink::try_new(&stream_handle).context("failed to create audio sink")?;

        Ok(Self {
            downloader,
            
            current: None,
            loaded: None,

            stream,
            stream_handle,
            sink
        })    
    }   

    pub async fn load(&mut self, url: &str) -> Result<()> {
        
        let video = self.downloader.fetch_video_infos(url.to_string())
            .await.context("failed to fetch video metadata")?;
        
        let filename = format!("{}.m4a", Self::sanitize_filename(&video.title));

        let path = self.downloader.download_audio_stream(&video, &filename)
            .await.context("failed to download audio stream")?;

        self.loaded = Some(Track {
            video,
            path,
        });

        let loaded_track = self.loaded.take().unwrap();
        let current_track = self.current.take().unwrap();

        if loaded_track.path != current_track.path {
            // If something was loaded, but wasn't ever played, delete it
            let _ = self.delete_file(loaded_track);
        }

        Ok(())
    }

    pub fn swap(&mut self) -> Result<()> {
        // a few steps here

        // 1. delete the current video and its artifacts
        // 2. swap in the loaded video into the current

        // delete the current video and artifacts

        let next = self.loaded.take().context("no track preloaded")?;

        self.discard_current_playback()?;

        let file = fs::File::open(&next.path)
            .with_context(|| format!("failed to open {}", next.path.display()))?;
        
        let source = Decoder::new(std::io::BufReader::new(file))
            .context("failed to decode audio file")?;

        self.sink.stop();
        self.sink.append(source);
        self.sink.pause();

        self.current = Some(next);

        Ok(())
    }

    fn discard_current_playback(&mut self) -> Result<()> {
        self.sink.stop();
        let current_track = self.current.take().unwrap();
        let _ = self.delete_file(current_track);

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
        self.sink.pause();
    }

    /// Resume a paused track.
    pub fn resume(&self) {
        self.sink.play();
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
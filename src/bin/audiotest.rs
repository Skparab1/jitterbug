use tokio::process::Command;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let video_id = "dQw4w9WgXcQ";
    
    println!("Testing yt-dlp MP3 download with cookies for {}...", video_id);

    let output = Command::new("yt-dlp")
        // Extract audio and convert to mp3
        .arg("-x")
        .arg("--audio-format")
        .arg("mp3")
        .arg("--audio-quality")
        .arg("0")
        // Pass cookies to bypass 403 Forbidden errors
        // Change "chrome" to "firefox", "edge", or "safari" if needed
        .arg("--cookies-from-browser")
        .arg("firefox") 
        // Output formatting
        .arg("-o")
        .arg("test_audio_%(id)s.%(ext)s")
        .arg(format!("https://www.youtube.com/watch?v={}", video_id))
        .output()
        .await?;

    if output.status.success() {
        println!("Success! The audio has been downloaded and converted to MP3.");
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Details:\n{}", stdout.trim());
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        eprintln!("yt-dlp failed with error:\n{}", error_msg);
    }

    Ok(())
}
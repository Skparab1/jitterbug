use rust_tcp::audio::Audio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

	let mut audio = Audio::new().await?;

    audio.play_test_file();

    loop {
        
    }
    Ok(())
}

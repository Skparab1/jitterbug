use rust_tcp::client::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

	let mut client = Client::new().await?;

	client.syn_handshake().await?;
	
	client.receive_instructions().await?;

	Ok(())
}

use rust_tcp::server::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut server = Server::new().await;
    server.run().await?;
    Ok(())
}
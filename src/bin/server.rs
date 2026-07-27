use rust_tcp::server::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut server = Server::new("127.0.0.1", 8080);
    server.run().await?;
    Ok(())
}
use tokio::net::UdpSocket;

pub struct Client {
    pub server_host: String,
    pub server_port: u16,
    pub socket: UdpSocket,
}

impl Client {
    pub async fn connect(host: String, port: u16) -> anyhow::Result<Self> {
        let host = host.into();
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        let local = socket.local_addr()?;
        Ok( Self{ server_host: host, server_port: port, socket})
    }

    pub async fn send_message(&self, message: String) -> anyhow::Result<()> {
        let address = format!("{}:{}", self.server_host, self.server_port);
        let message_bytes = message.as_bytes();
        self.socket.send_to(&message_bytes, address).await?;
        Ok(())
    }

    pub async fn send_message_bytes(&self, message_bytes: &[u8]) -> anyhow::Result<()> {
        let address = format!("{}:{}", self.server_host, self.server_port);
        self.socket.send_to(&message_bytes, address).await?;
        Ok(())
    }

    pub async fn receive_bytes(&self, buffer: &mut [u8]) -> anyhow::Result<(usize, std::net::SocketAddr)> {
        Ok(self.socket.recv_from(buffer).await?)
    }
}
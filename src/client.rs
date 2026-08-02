// Core
use tokio::net::UdpSocket;
use std::io;
use uuid::Uuid;

// Crypto and encoding
use rand::thread_rng;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs8::DecodePublicKey};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

// Constants and in-house utils
use crate::constants::packet_types;
use crate::utils::{create_frame, validate_received_datagram, extract_payload};


pub struct Client {
    // client constants
    server_host: String,
    server_port: u16,
    socket: UdpSocket,
    uuid: Uuid,

    // client variables
    sequence_number: u32,
}

impl Client {
    pub async fn new() -> anyhow::Result<Self> {
        let mut address_port = String::new();

        println!("Please enter the server address and port as <address>:<port>  >");
        io::stdin().read_line(&mut address_port).expect("failed to readline");

        let split_address_port: Vec<&str> = address_port.trim().split(':').collect();
        let port = split_address_port[1].parse::<u16>().expect("failed to parse port");
        let address = split_address_port[0].to_string();

        Self::connect(address, port).await
    }

    async fn connect(host: String, port: u16) -> anyhow::Result<Self> {
        let host = host.into();
        let uuid = Uuid::new_v4();
        
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        Ok( Self{ server_host: host, server_port: port, socket, uuid, sequence_number: 0} )
    }

    pub async fn syn_handshake(&mut self) -> anyhow::Result<()> {
        println!("Please enter the server public key  >");
        let mut input_key = String::new();	
        io::stdin().read_line(&mut input_key).expect("failed to readline");

        // we only need the public key once: for the handshake. Thus not stored as a field.
        let der = STANDARD.decode(input_key.trim())?;
        let public_key = RsaPublicKey::from_public_key_der(&der)?;

        self.offer_handshake(public_key).await?;

        // now wait for the response

        let mut buffer = [0u8; 264];
        let (bytes_read, _sender_addr) = self.receive_bytes(&mut buffer).await?;

        validate_received_datagram(&buffer[..bytes_read], self.sequence_number + 1, packet_types::CONNECTION_ACK);

        println!("Connected!");

        self.sequence_number += 1;

        Ok(())

    }   

    async fn offer_handshake(&mut self, public_key: RsaPublicKey) -> anyhow::Result<()> {
        // first, we create our UUID payload
        let data = self.uuid.to_string().into_bytes();
        let mut rng = thread_rng();
        let enc_data = public_key
            .encrypt(&mut rng, Pkcs1v15Encrypt, &data[..])
            .expect("Failed to encrypt data");

        // construct the frame of the message
        let encrypted_key_bytes = enc_data.clone(); // the actual payload

        let seq_bytes = self.sequence_number.to_be_bytes(); // 4 bytes

        let frame = create_frame(packet_types::CONNECTION_SYN, &seq_bytes, &encrypted_key_bytes, None).await?;

        self.send_message_bytes(&frame).await?;

        self.sequence_number += 1;

        println!("\n\nYour UUID is: {}", self.uuid.to_string());
	    println!("Connecting to server... ");

        Ok(())
    }

    pub async fn receive_instructions(&mut self) -> anyhow::Result<()> {
        let mut buffer = [0u8; 264];
        loop {
            let (bytes_read, sender_addr) = self.receive_bytes(&mut buffer).await?;

            println!("Received datagram from {}: {} bytes", sender_addr, bytes_read);

            let payload: Vec<u8> = extract_payload(&buffer[..bytes_read], self.sequence_number + 1, &self.uuid, packet_types::MISC).expect("Payload extraction failed");

            let decoded: &str = std::str::from_utf8(&payload)?;

            println!("Payload was {}", decoded);

            self.sequence_number += 1;
        }
    }

    async fn send_message_bytes(&self, message_bytes: &[u8]) -> anyhow::Result<()> {
        let address = format!("{}:{}", self.server_host, self.server_port);
        self.socket.send_to(&message_bytes, address).await?;
        Ok(())
    }

    pub async fn receive_bytes(&self, buffer: &mut [u8]) -> anyhow::Result<(usize, std::net::SocketAddr)> {
        Ok(self.socket.recv_from(buffer).await?)
    }
}
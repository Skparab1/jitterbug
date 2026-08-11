use tokio::net::UdpSocket;
use tokio::io::{self, BufReader, AsyncBufReadExt};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use rsa::{RsaPrivateKey, RsaPublicKey, pkcs8::EncodePublicKey, rand_core::OsRng};

use crate::constants::{packet_types, ConnectionState, SERVER_HOST, SERVER_PORT};

use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

use crate::utils::{validate_received_datagram, send_datagram};
use crate::audio::Audio;

pub struct Server {

    // constants
    pub host: String,
    pub port: u16,

    // connections
    pub connections: HashMap<SocketAddr, ConnectionState>,
    pub listener: UdpSocket,

    // crypto
    private_key: RsaPrivateKey,
    pub public_key: RsaPublicKey,

    audio: Audio,
}

impl Server {
    pub async fn new() -> Self {
        println!("Server is spinning up...\n\n");

        let mut rng = OsRng;

        let listener = UdpSocket::bind(format!("{}:{}", SERVER_HOST, SERVER_PORT)).await.expect("Socket binding failed");
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("Private Key generation failed");
        let public_key = RsaPublicKey::from(&private_key);

        let audio = Audio::new().await.expect("Initializing server side audio failed.");
        
        Self {
            host: SERVER_HOST.into(),
            port: SERVER_PORT,
            connections: HashMap::new(),

            // these are placeholders, will be overwritten later
            listener,
            private_key,
            public_key,

            audio,
        }
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {

        println!("Address: {}:{}\n", self.host, self.port);

        let der = self.public_key.to_public_key_der()?;
        println!("Public key: {}\n\n", STANDARD.encode(der.as_ref()));


        Ok(())
    }

    pub async fn send_datagram_to_client(&mut self, 
        recipient: &SocketAddr, 
        packet_type: u8,
        payload: &[u8]) -> anyhow::Result<()>{

        let connection = self
            .connections
            .get_mut(recipient)
            .ok_or_else(|| anyhow::anyhow!("recipient not found"))?;       
        
        connection.sequence_number += 1;
        let seq_bytes = connection.sequence_number.to_be_bytes();
        let uuid_bytes = *connection.uuid.as_bytes();

        let _ = send_datagram(&self.listener, recipient, packet_type, &seq_bytes, &uuid_bytes, payload).await;

        Ok(())
    }



    async fn receive_connection(&mut self, buffer: [u8; 264], bytes_read: usize, sender_addr: SocketAddr) -> anyhow::Result<()> {

        // for now, we say that the only types of packets the server receives are connection-syn
        // this may change if we make it a two-way communication
        validate_received_datagram(&buffer[..bytes_read], 0, packet_types::CONNECTION_SYN);

        if bytes_read < 264 {
            println!("Invalid packet size for connection request");
            return Ok(());
        }

        let encrypted_uuid = &buffer[8..264];
        let uuid_bytes = self.private_key.decrypt(rsa::Pkcs1v15Encrypt, encrypted_uuid)?;
        let uuid_string = std::str::from_utf8(&uuid_bytes)?;
        let uuid = Uuid::parse_str(uuid_string)?;

        let entry = self.connections.entry(sender_addr).or_insert(ConnectionState {
            uuid,
            sequence_number: 1, // just the syn
        });

        println!("UUID: {} \t\t address: {}", uuid, sender_addr);

        // send an ack back.
        entry.sequence_number += 1;
        send_datagram(&self.listener, &sender_addr, packet_types::CONNECTION_ACK, &entry.sequence_number.to_be_bytes(), &[], &[]).await?;
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        println!("Listening for connections...\n");

        let mut buffer = [0u8; 264];
        let mut stdin_lines = BufReader::new(io::stdin()).lines();

        loop {
            tokio::select! {
                
                result = self.listener.recv_from(&mut buffer) => {
                    let (bytes_read, sender_addr) = result?;
                    self.receive_connection(buffer, bytes_read, sender_addr).await?;
                }

                line = stdin_lines.next_line() => {
                    if let Some(line) = line? {
                        println!("You typed {}", line);

                        if line == "quit" {
                            break Ok(())
                        }

                        let mut send_packet_type = packet_types::MISC;
                        let mut payload = "".as_bytes();

                        if line.starts_with("load ") {
                            send_packet_type = packet_types::AUDIO_LOAD;
                            payload = line[5..].as_bytes();
                        } else if line.starts_with("swap") {
                            send_packet_type = packet_types::AUDIO_SWAP;
                        } else if line.starts_with("play") {
                            send_packet_type = packet_types::AUDIO_PLAY;
                        } else if line.starts_with("pause") {
                            send_packet_type = packet_types::AUDIO_PAUSE;
                        } else if line.starts_with("forward") {
                            send_packet_type = packet_types::AUDIO_FWD;
                        } else if line.starts_with("backward") {
                            send_packet_type = packet_types::AUDIO_BACK;
                        }

                        if !self.audio.preflight_check(send_packet_type)? {
                            println!("Command did not pass Audio's preflight checks");
                            continue;
                        }

                        let recipients: Vec<SocketAddr> = self.connections.keys().copied().collect();
                        for recipient in recipients {
                            let _ = self.send_datagram_to_client(&recipient, send_packet_type, &payload).await;
                        }

                        // send it to our own audio module
                        if line.starts_with("load ") {
                            self.audio.load(line[5..].as_ref()).await?;
                        } else if line.starts_with("swap") {
                            let _ = self.audio.swap();
                        } else if line.starts_with("play") {
                            self.audio.play();
                        } else if line.starts_with("pause") {
                            self.audio.pause();
                        } else if line.starts_with("forward") {
                            self.audio.forward();
                        } else if line.starts_with("backward") {
                            self.audio.backward();
                        }
                    }
                }
            }
        }      
    }
}
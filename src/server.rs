use tokio::net::UdpSocket;
use tokio::io::{self, AsyncBufReadExt, BufReader};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use rsa::{RsaPrivateKey, RsaPublicKey, pkcs8::EncodePublicKey, rand_core::OsRng};

use crate::constants::{MAGIC_BYTES, packet_types, ConnectionState};

use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

pub struct Server {
    pub host: String,
    pub port: u16,
    pub connections: HashMap<SocketAddr, ConnectionState>
}

impl Server {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Server {
            host: host.into(),
            port,
            connections: HashMap::new()
        }
    }

    // pub async fn reply(&mut self, listener: UdpSocket, recipient: impl Into<String>, message: impl Into<String>){
    //     listener.send_to(message.into().as_bytes(), recipient.into()).await?;
    // }

    // this function will determine the sequence
    pub async fn send_instruction(&mut self, 
        listener: &UdpSocket, 
        recipient: &SocketAddr, 
        packet_type: u8,
        payload: &[u8]
    ) -> anyhow::Result<()> {
        // we can build the datagram frame here.

        println!("Building frame for {}", recipient);

        let mut frame: Vec<u8> = Vec::new();
        frame.extend_from_slice(&MAGIC_BYTES);
        frame.extend_from_slice(&[packet_type]);

        println!("Building frame for {}", recipient);
        
        let connection = self
            .connections
            .get_mut(recipient)
            .ok_or_else(|| anyhow::anyhow!("recipient not found"))?;       
        
        connection.sequence_number += 1;

        let seq_bytes = connection.sequence_number.to_be_bytes();

        frame.extend_from_slice(&seq_bytes);
        let uuid_bytes = connection.uuid.as_bytes();
        frame.extend_from_slice(uuid_bytes);

        frame.extend_from_slice(&payload);

        println!("Sending frame {:02X?} for {}", frame, recipient);

        listener.send_to(&frame, recipient).await?;
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        println!("Server is spinning up...\n\n");

        let listener = UdpSocket::bind(format!("{}:{}", self.host, self.port)).await?;
        println!("Address: {}:{}\n", self.host, self.port);

        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
        let public_key = RsaPublicKey::from(&private_key);

        let der = public_key.to_public_key_der()?;
        println!("Public key: {}\n\n", STANDARD.encode(der.as_ref()));

        
        println!("Connected to:\n");


        let mut buffer = [0u8; 264];

        let mut stdin_lines = BufReader::new(io::stdin()).lines();

        loop {

            tokio::select! {

                result = listener.recv_from(&mut buffer) => {
                    let (bytes_read, sender_addr) = result?;
            
                    let magic_bytes = &buffer[0..3];
                    
                    // println!("Received magic bytes: {:02X?} \nfrom {}", magic_bytes, sender_addr);

                    if (magic_bytes != MAGIC_BYTES) {
                        println!("Message not intended for us, discarding.");
                        continue;
                    }

                    let packet_type = buffer[3];
                    // println!("Received packet type: {:02X?}", packet_type);

                    let sequence_number = u32::from_be_bytes(buffer[4..8].try_into()?);
                    
                    // println!("Received sequence number: {}", sequence_number);

                    // here on out, the payload will be different depending on the packet type

                    if (packet_type == packet_types::CONNECTION_SYN) {
                        // first thing we check is that seq is 0
                        if (sequence_number != 0) {
                            println!("Invalid sequence number for initial connection");
                            continue;
                        }

                        // its fine then. grab the uuid and decrypt it
                        if (bytes_read < 264) {
                            println!("Invalid packet size for connection request");
                            continue;
                        }
                        let encrypted_UUID = &buffer[8..264];

                        let UUID_bytes = private_key.decrypt(rsa::Pkcs1v15Encrypt, encrypted_UUID)?;

                        let uuid_string = std::str::from_utf8(&UUID_bytes)?;
                        let uuid = Uuid::parse_str(uuid_string)?;

                        let entry = self.connections.entry(sender_addr).or_insert(ConnectionState {
                            uuid,
                            sequence_number: 1,
                        });

                        println!("UUID: {} \t\t address: {}", uuid, sender_addr);

                        
                        // send an ack back.

                        let mut frame: Vec<u8> = Vec::new();
                        frame.extend_from_slice(&MAGIC_BYTES);
                        frame.extend_from_slice(&[packet_types::CONNECTION_ACK]);
                        frame.extend_from_slice(&entry.sequence_number.to_be_bytes());
                        // that's kinda it, no payload for this one.


                        // println!("Sending connection ack to {} with sequence number: {}", sender_addr, entry.sequence_number);
                        // println!("Sending {:02X?} bytes to {}", frame, sender_addr);


                        listener.send_to(&frame, sender_addr).await?;
                    }
                }

                line = stdin_lines.next_line() => {
                    if let Some(line) = line? {
                        println!("You typed {}", line);

                        let recipients: Vec<SocketAddr> = self.connections.keys().copied().collect();

                        for recipient in recipients {
                            println!("Attempt to send to {}", recipient);
                            self.send_instruction(&listener, &recipient, packet_types::MISC, line.as_bytes()).await?;
                        }
                    }

                }
            }

            

            //     println!("{}", byte);
            // }

            // let message = String::from_utf8_lossy(&buffer[..bytes_read]);
            // println!("Received content: {} \nfrom {}", message, sender_addr);
        }        
    }
}
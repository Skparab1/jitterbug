// Core
use tokio::net::UdpSocket;
use std::io;
use crate::audio::Audio;

// Crypto and encoding
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

// Constants and in-house utils
use crate::constants::packet_types;
use crate::utils::{create_frame, extract_payload, packet_type_to_text};

use aes_gcm::{
    aead::{ Generate, Key, KeyInit},
    Aes128Gcm,
};

pub struct Client {
    // client constants
    server_host: String,
    server_port: u16,
    socket: UdpSocket,

    // client variables
    sequence_number: u32,

    // cryptography
    cipher: Aes128Gcm,

    // audio element
    audio: Audio,
}

impl Client {
    pub async fn new() -> anyhow::Result<Self> {

        Self::connect().await
    }

    // potentially reorganize into one function

    async fn connect() -> anyhow::Result<Self> {

        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        println!("Please enter the public sharing key");
        let mut input_key = String::new();
        io::stdin().read_line(&mut input_key).expect("failed to readline");

        let decoded_bytes = STANDARD.decode(input_key.trim())?;
        let decoded_string = String::from_utf8(decoded_bytes)?;

        let Some((address, key)) = decoded_string.split_once("||") else { todo!() };
        let Some((host, port)) = address.split_once(':') else { todo!() };
        
        let decoded_key = STANDARD.decode(key).expect("Failed to decode key");
        let pre_shared_key = Key::<Aes128Gcm>::from_slice(decoded_key.as_slice());
        let cipher = Aes128Gcm::new(pre_shared_key);

        let server_host = host.to_string();
        let server_port = port.parse::<u16>().expect("Failed to parse port from sharing key");

        let audio = Audio::new().await?;

        Ok( Self{ server_host, server_port, socket, sequence_number: 0, cipher, audio } )
    }

    pub async fn syn_handshake(&mut self) -> anyhow::Result<()> {
        
        let nonce = self.offer_handshake().await?;

        // now wait for the response

        let mut buffer = [0u8; 256];
        let (bytes_read, _sender_addr) = self.receive_bytes(&mut buffer).await?;

        let response: Option<Vec<u8>> = extract_payload(&self.cipher, &buffer[..bytes_read], self.sequence_number + 1, packet_types::CONNECTION_ACK);

        if response.is_none() {
            println!("Error: Handshake failed, internal");
            return Err(anyhow::anyhow!("Error: Handshake failed, internal"));
        }

        let got_nonce = &response.unwrap()[5..];

        if got_nonce != nonce.as_slice() {
            println!("Error: Handshake failed, nonce mismatch");
            println!("Expected: {:?}", nonce.as_slice());
            println!("Received: {:?}", got_nonce);
            return Err(anyhow::anyhow!("Error: Handshake failed, nonce mismatch"));
        }

        println!("Connected!");

        self.sequence_number += 1;
        
        Ok(())

    }   

    // we have this return the nonce, so that the upstream function can verify whatever is returned by the server.
    async fn offer_handshake(&mut self) -> anyhow::Result<aes_gcm::aead::Nonce<Aes128Gcm>> {
        // we generate a nonce
        let challenge_nonce = aes_gcm::aead::Nonce::<Aes128Gcm>::generate();

        // construct the frame of the message
        let seq_bytes = self.sequence_number.to_be_bytes(); // 4 bytes

        let frame = create_frame(&self.cipher, packet_types::CONNECTION_SYN, &seq_bytes, Some(challenge_nonce.as_slice())).await?;

        self.send_message_bytes(&frame).await?;

        self.sequence_number += 1;

	    println!("Connecting to server... ");

        Ok(challenge_nonce)
    }

    pub async fn receive_instructions(&mut self) -> anyhow::Result<()> {
        let mut buffer = [0u8; 264];
        loop {
            let (bytes_read, sender_addr) = self.receive_bytes(&mut buffer).await?;

            println!("Received datagram of length {} from {}", bytes_read, sender_addr);

            let response: Option<Vec<u8>> = extract_payload(&self.cipher, &buffer[..bytes_read], self.sequence_number + 1, packet_types::AUDIO_ANY);

            if response.is_some() {
                let payload = response.unwrap();
                
                println!("Payload length: {}", payload.len());

                let packet_type = payload[0];
                let packet_text = packet_type_to_text(packet_type);

                println!("Received datagram of type {} from {}", packet_text, sender_addr);

                self.sequence_number += 1;

                self.action_audio_instruction(packet_type, payload[5..].to_vec()).await?;
           
            } else {
                println!("Error: Failed to extract payload from datagram");
            }        
        }
    }

    pub async fn action_audio_instruction(&mut self, packet_type: u8, payload: Vec<u8>) -> anyhow::Result<()> {
        if packet_type == packet_types::AUDIO_PLAY {
            // here, 32 bytes.
            // the first 16 bytes: what to set the rodio player timestamp to
            // the second 16: when exactly (unit timestamp wise) to play the audio

            let to_set_timestamp = u128::from_be_bytes(payload[0..16].try_into()?);

            println!("The timestamp to set the player to is: {}", to_set_timestamp);
            let when_to_play_timestamp = u128::from_be_bytes(payload[16..32].try_into()?);

            println!("The timestamp to play at is: {}", when_to_play_timestamp);
            
            self.audio.play_at(to_set_timestamp, when_to_play_timestamp).await;

        } else if packet_type == packet_types::AUDIO_PAUSE {
            self.audio.pause();
        } else if packet_type == packet_types::AUDIO_FWD {
            self.audio.forward();
        } else if packet_type == packet_types::AUDIO_BACK {
            self.audio.backward();
        } else if packet_type == packet_types::AUDIO_PAUSE {
            self.audio.pause();
        } else if packet_type == packet_types::AUDIO_SWAP {
            println!("Swapping audio track");
            if let Err(err) = self.audio.swap() {
                println!("audio swap failed: {err:#}");
            }
        } else if packet_type == packet_types::AUDIO_LOAD {
            let decoded: &str = std::str::from_utf8(&payload)?;

            let _ = self.audio.load(decoded).await;
            // after it loads, send the ack.

            let frame = create_frame(&self.cipher, packet_types::LOADED_ACK, &self.sequence_number.to_be_bytes(), None).await?;
            self.send_message_bytes(&frame).await?;

            self.sequence_number += 1;
        } else if packet_type == packet_types::AUDIO_VOL {
            let vol_level = u128::from_be_bytes(payload[0..16].try_into()?);
            self.audio.set_volume(vol_level);
        }

        return Ok(());
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
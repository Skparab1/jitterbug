use tokio::net::UdpSocket;
use tokio::io::{self, BufReader, AsyncBufReadExt};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use crate::constants::{packet_types, ConnectionState, SERVER_HOST, SERVER_PORT};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;

use crate::utils::{extract_payload, send_datagram};
use crate::audio::Audio;

use crate::tui::SimpleUI;
use tokio::sync::watch;


// clean up the imports later

use aes_gcm::{
    aead::{Generate, Key, KeyInit},
    Aes128Gcm,
};

pub struct Server {

    // constants
    pub host: String,
    pub port: u16,

    // connections
    pub connections: HashMap<SocketAddr, ConnectionState>,
    pub listener: UdpSocket,

    // crypto
    pub cipher: Aes128Gcm,

    audio: Audio,

    ui: SimpleUI,
}

impl Server {
    pub async fn new() -> Self {
        let listener = UdpSocket::bind(format!("{}:{}", SERVER_HOST, SERVER_PORT)).await.expect("Socket binding failed");

        let pre_shared_key = Key::<Aes128Gcm>::generate();
        let cipher = Aes128Gcm::new(&pre_shared_key);
    
        let sharing_key = STANDARD.encode(format!("{}:{}||{}", SERVER_HOST, SERVER_PORT, STANDARD.encode(pre_shared_key)));

        let audio = Audio::new("server-temp-assets".to_string()).await.expect("Initializing server side audio failed.");

        let ui = SimpleUI::new(sharing_key);

        Self {
            host: SERVER_HOST.into(),
            port: SERVER_PORT,
            connections: HashMap::new(),

            listener,
            cipher,

            audio,

            ui: ui.expect("UI failed to initialize"),
        }
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

        let _ = send_datagram(&self.cipher, &self.listener, recipient, packet_type, &seq_bytes, payload).await;

        Ok(())
    }


    async fn process_received_datagram(&mut self, buffer: [u8; 264], bytes_read: usize, sender_addr: SocketAddr) -> anyhow::Result<()> {
        
        // so we know the address right
        // based on that we can predict
        // whether it should be a connection syn
        // or a loaded ack or something else

        if !self.connections.contains_key(&sender_addr){
            // likely a connection syn 
            // we treat it as one
            // println!("Received connection request from {}", sender_addr);
            self.receive_connection(buffer, bytes_read, sender_addr).await?;

        } else {
            // almost certainly a loaded ack.            
            self.receive_loaded_ack(buffer, bytes_read, sender_addr).await?;
        }

        Ok(())
    }

    async fn receive_loaded_ack(&mut self, buffer: [u8; 264], bytes_read: usize, sender_addr: SocketAddr) -> anyhow::Result<()> {

        // figure out which connection it is 

        if self.connections.contains_key(&sender_addr){
            let state: Option<&mut ConnectionState> = self.connections.get_mut(&sender_addr);
            if state.is_some() {
                let rstate = state.unwrap();

                // we don't care about the payload here
                let _ = extract_payload(&self.cipher, &buffer[..bytes_read], packet_types::LOADED_ACK, rstate.sequence_number);

                rstate.acked_signal = true;
                rstate.sequence_number += 1;

                // println!("Client {} has loaded the track.", sender_addr);
                self.ui.render_current_clients(&self.connections);

                return Ok(())
            }
        }

        println!("Received a loaded ack datagram from an unregistered client");

        Ok(())
    }


    async fn receive_connection(&mut self, buffer: [u8; 264], bytes_read: usize, sender_addr: SocketAddr) -> anyhow::Result<()> {

        let content = extract_payload(&self.cipher, &buffer[..bytes_read], packet_types::CONNECTION_SYN, 0);

        if content.is_some(){
            let unwrapped_content = content.unwrap();

            let received_nonce = unwrapped_content[5..].to_vec(); // the first 5 bytes are packet type and seq number, the rest is the nonce

            // a bit strange but just keep the mutable borrow within a narrower scope
            let sequence_number = {
                let entry = self.connections.entry(sender_addr).or_insert(ConnectionState {
                    sequence_number: 1, // just the syn
                    acked_signal: false,
                });
                
                entry.sequence_number += 1;

                send_datagram(&self.cipher, &self.listener, &sender_addr, packet_types::CONNECTION_ACK, &entry.sequence_number.to_be_bytes(), &received_nonce).await?;

                entry.sequence_number
            }; 

            self.ui.render_current_clients(&self.connections);

            Ok(())

        } else {
            println!("Error receiving connection");
            Ok(())
        }
    }

    async fn select_action(&mut self, line: String) -> anyhow::Result<()> {

        let mut send_packet_type = packet_types::MISC;
        let mut payload: Vec<u8> = Vec::new();

        if line.starts_with("load ") {
            send_packet_type = packet_types::AUDIO_LOAD;
            payload.extend_from_slice(&line[5..].as_bytes());
        } else if line.starts_with("swap") {
            send_packet_type = packet_types::AUDIO_SWAP;
        } else if line.starts_with("play") {
            send_packet_type = packet_types::AUDIO_PLAY;
            // need to record the audio timestamp where to start playing
            
            let current_pos = self.audio.get_pos();
            let current_pos_bytes = current_pos.as_millis().to_be_bytes();
            payload.extend_from_slice(&current_pos_bytes);

            let play_time = std::time::SystemTime::now() + std::time::Duration::from_millis(500);

            let play_time_millis = play_time.duration_since(std::time::UNIX_EPOCH)?.as_millis();
            let play_time_bytes = play_time_millis.to_be_bytes();

            payload.extend_from_slice(&play_time_bytes);

        } else if line.starts_with("pause") {
            send_packet_type = packet_types::AUDIO_PAUSE;
        } else if line.starts_with("forward") {
            send_packet_type = packet_types::AUDIO_FWD;
        } else if line.starts_with("backward") {
            send_packet_type = packet_types::AUDIO_BACK;
        } else if line.starts_with("vol ") {

            send_packet_type = packet_types::AUDIO_VOL;
            
            let vol_str = &line[4..];

            // I have entered a 0.5 too many times
            if vol_str.contains('.') {
                println!("Volume level cannot be a decimal.");
                return Ok(());
            }

            let vol_level = u128::from_str(vol_str).expect("Invalid volume level");
            if vol_level > 100  {
                println!("Volume level must be between 0 and 100");
                return Ok(());
            }

            payload.extend_from_slice(&vol_level.to_be_bytes());

        }

        if !self.audio.preflight_check(send_packet_type)? {
            println!("Command did not pass Audio's preflight checks");
            return Ok(());
        }

        let recipients: Vec<SocketAddr> = self.connections.keys().copied().collect();
        for recipient in recipients {
            let _ = self.send_datagram_to_client(&recipient, send_packet_type, &payload).await;
            let state = self.connections.get_mut(&recipient);

            if state.is_some(){
                state.unwrap().acked_signal = false;
            }
        }

        // send it to our own audio module
        if line.starts_with("load ") {
            self.ui.set_status("Loading track...");
            self.ui.update_queue(self.audio.get_queue_titles(), self.audio.get_current_track_title().unwrap_or_default(), line[5..].to_string().into());
            self.audio.load(line[5..].as_ref()).await?;
            self.ui.update_queue(self.audio.get_queue_titles(), self.audio.get_current_track_title().unwrap_or_default(), None);
        } else if line.starts_with("swap") {
            let _ = self.audio.swap();
            self.ui.update_queue(self.audio.get_queue_titles(), self.audio.get_current_track_title().unwrap_or_default(), None);
        } else if line.starts_with("play") {
            let to_set_timestamp = u128::from_be_bytes(payload[0..16].try_into().expect("slice with incorrect length"));
            let when_to_play_timestamp = u128::from_be_bytes(payload[16..32].try_into().expect("slice with incorrect length"));
            
            self.audio.play_at(to_set_timestamp, when_to_play_timestamp).await;

        } else if line.starts_with("pause") {
            self.audio.pause();
        } else if line.starts_with("forward") {
            self.audio.forward();
        } else if line.starts_with("backward") {
            self.audio.backward();
        } else if line.starts_with("vol ") {
            let vol_level = u128::from_str(&line[4..]).expect("Invalid volume level");
            self.audio.set_volume(vol_level);
        }

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.ui.set_status("Listening for connections ...");

        let mut buffer = [0u8; 264];
        let mut stdin_lines = BufReader::new(io::stdin()).lines();

        loop {
            tokio::select! {
                
                result = self.listener.recv_from(&mut buffer) => {
                    let (bytes_read, sender_addr) = result?;
                    self.process_received_datagram(buffer, bytes_read, sender_addr).await?;
                }

                _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {
                    if let Some(line) = self.ui.poll_command()? {
                        // Print to your TUI's output box instead of standard println!
                        self.ui.set_status(format!("You typed: {}", line));

                        if line == "quit" {
                            break Ok(());
                        }

                        let _ = self.select_action(line).await;
                    }
                }
            }
        }      
    }
}
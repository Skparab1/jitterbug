use crate::constants::{MAGIC_BYTES, packet_types};
use uuid::Uuid;
use tokio::net::UdpSocket;
use std::net::SocketAddr;

// common cryptography
use aes_gcm::{
    aead::{Aead, AeadCore, Generate, Key, KeyInit},
    Aes128Gcm, Nonce,
};

pub fn extract_payload(
	cipher: &Aes128Gcm,
	datagram: &[u8],
	expected_seq: u32,
	packet_type: u8,
) -> Option<&[u8]> {
	// check magic bytes
	let magic_bytes = &datagram[0..4];
	if magic_bytes != MAGIC_BYTES {
		println!("Message not intended for us, discarding.");
		return None;
	}

	// check the nonce. we will need this to decrypt
	let nonce = &datagram[4..16];

	let mut content = &datagram[16..];

	cipher.decrypt_in_place(&nonce, b"", &mut content)?;

	// The content now contains packet type (1b), seq (4b), and payload.

    if packet_type != content[0] && packet_type != packet_types::AUDIO_ANY {
        println!("Packet type does not match");
        return None;
    }

	if packet_type == packet_types::AUDIO_ANY && 
		content[0] != packet_types::AUDIO_LOAD && 
		content[0] != packet_types::AUDIO_SWAP && 
		content[0] != packet_types::AUDIO_PLAY && 
		content[0] != packet_types::AUDIO_PAUSE &&
		content[0] != packet_types::AUDIO_FWD &&
		content[0] != packet_types::AUDIO_BACK &&
		content[0] != packet_types::AUDIO_VOL &&
		content[0] != packet_types::MISC { // for now, we allow misc packets. This may change later.
        println!("Packet type does not match");
        return None;
    }

	if content.len() < 5 {
		println!("Content too short");
		return None;
	}

	let seq_bytes: [u8; 4] = match content[1..5].try_into() {
		Ok(bytes) => bytes,
		Err(_) => {
			println!("Failed to convert sequence bytes");
			return None;
		}
	};
	
	let sequence_number: u32 = u32::from_be_bytes(seq_bytes);

	if sequence_number != expected_seq {
		println!("Sequence number of received datagram was not an increment.");
		println!("Got {}, expected {}", sequence_number, expected_seq);
		return None;
	}    

	let payload = content.len() > 5 ? &content[5] : &[]; // notably: not the same as none

	return Some(payload);
}

pub async fn create_frame(
	cipher: &Aes128Gcm,
	packet_type: u8,		 // 1 byte
    sequence_number: &[u8],	 // 4 bytes
    nonce: &[u8],            // 12 bytes
    payload: Option<&[u8]>,  
) -> anyhow::Result<Vec<u8>> {
	let mut frame: Vec<u8> = Vec::new();

	let mut content: Vec<u8> = Vec::new();
	content.push(packet_type);
	content.extend_from_slice(sequence_number);
	content.extend_from_slice(payload.unwrap_or(&[]));

	cipher.encrypt_in_place(&nonce, b"", &mut content)?;

	frame.extend_from_slice(&MAGIC_BYTES);
    frame.extend_from_slice(nonce);
	frame.extend_from_slice(&content);

	Ok(frame)
}

pub async fn send_datagram( 
	cipher: &Aes128Gcm,
    listener: &UdpSocket, 
    recipient: &SocketAddr, 
    packet_type: u8,
    sequence_number: &[u8],
    payload: &[u8],
) -> anyhow::Result<()> {
	let nonce = Nonce::generate();
    
    let frame = create_frame(cipher, packet_type, sequence_number, nonce, Some(payload)).await?;

    listener.send_to(&frame, recipient).await?;
    Ok(())
}

pub fn packet_type_to_text(packet_type: u8) -> &'static str {
    match packet_type {
        packet_types::CONNECTION_SYN => "Connection SYN",
        packet_types::CONNECTION_ACK => "Connection ACK",
        packet_types::MISC => "Misc",
        packet_types::AUDIO_LOAD => "Audio Load",
        packet_types::AUDIO_SWAP => "Audio Swap",
        packet_types::AUDIO_PLAY => "Audio Play",
        packet_types::AUDIO_PAUSE => "Audio Pause",
        packet_types::AUDIO_FWD => "Audio Forward",
        packet_types::AUDIO_BACK => "Audio Back",
		packet_types::AUDIO_VOL => "Audio Volume",
        _ => "Unknown",
    }
}
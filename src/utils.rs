use crate::constants::{MAGIC_BYTES, packet_types};
use uuid::Uuid;
use tokio::net::UdpSocket;
use std::net::SocketAddr;

pub fn validate_received_datagram(
	datagram: &[u8],
	expected_seq: u32,
	packet_type: u8,
) -> bool {
	// check magic bytes
	let magic_bytes = &datagram[0..3];
	if magic_bytes != MAGIC_BYTES {
		println!("Message not intended for us, discarding.");
		return false;
	}

    if packet_type != datagram[3] && packet_type != packet_types::AUDIO_ANY {
        println!("Packet type does not match");
        return false;
    }

	if packet_type == packet_types::AUDIO_ANY && 
		datagram[3] != packet_types::AUDIO_LOAD && 
		datagram[3] != packet_types::AUDIO_SWAP && 
		datagram[3] != packet_types::AUDIO_PLAY && 
		datagram[3] != packet_types::AUDIO_PAUSE &&
		datagram[3] != packet_types::AUDIO_FWD &&
		datagram[3] != packet_types::AUDIO_BACK &&
		datagram[3] != packet_types::MISC { // for now, we allow misc packets. This may change later.
        println!("Packet type does not match");
        return false;
    }

	if datagram.len() < 8 {
		println!("Datagram too short");
		return false;
	}

	let seq_bytes: [u8; 4] = match datagram[4..8].try_into() {
		Ok(bytes) => bytes,
		Err(_) => {
			println!("Failed to convert sequence bytes");
			return false;
		}
	};
	
	let sequence_number: u32 = u32::from_be_bytes(seq_bytes);

	if sequence_number != expected_seq {
		println!("Sequence number of received datagram was not an increment.");
		println!("Got {}, expected {}", sequence_number, expected_seq);
		return false;
	}    

	return true;
}

pub fn extract_payload(
	datagram: &[u8],
	current_seq: u32,
	uuid_param: &Uuid,
	expected_type: u8
) -> anyhow::Result<Vec<u8>> {
	// first check whether it is well-formatted for us or not.
	if !validate_received_datagram(datagram, current_seq, expected_type){
		// return an error
		return Err(anyhow::anyhow!("Invalid datagram received"))
	}

	// is it a connection syn or ack? if so, then it won't have a uuid as a part of the payload
	let packet_type = datagram[3];
	let mut payload_start: usize = 8;
	if packet_type != packet_types::CONNECTION_SYN && 
		packet_type != packet_types::CONNECTION_ACK {
		
		// check the uuid
		let received_uuid = &datagram[8..24];
		let uuid_bytes = uuid_param.as_bytes();
		// println!("Expected UUID: {:02X?}", uuid_bytes);
		// println!("Received UUID: {:02X?}", received_uuid);
		if uuid_bytes != received_uuid {
			return Err(anyhow::anyhow!("UUID does not match"))
		}

		payload_start = 24;
	}

	// Extract the payload from the datagram
	let payload = &datagram[payload_start..];
	return Ok(payload.to_vec())
}


pub async fn create_frame(
	packet_type: u8,
    sequence_number: &[u8],
    uuid_bytes: &[u8],
    payload: Option<&[u8]>,
) -> anyhow::Result<Vec<u8>> {
	let mut frame: Vec<u8> = Vec::new();

	frame.extend_from_slice(&MAGIC_BYTES);
    frame.extend_from_slice(&[packet_type]);    
    frame.extend_from_slice(&sequence_number);
    frame.extend_from_slice(uuid_bytes);
	if payload.is_some(){
    	frame.extend_from_slice(&payload.unwrap());
	}

	Ok(frame)
}

pub async fn send_datagram( 
    listener: &UdpSocket, 
    recipient: &SocketAddr, 
    packet_type: u8,
    sequence_number: &[u8],
    uuid_bytes: &[u8],
    payload: &[u8],
) -> anyhow::Result<()> {
    
    let frame = create_frame(packet_type, sequence_number, uuid_bytes, Some(payload)).await?;

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
        _ => "Unknown",
    }
}
use crate::constants::{MAGIC_BYTES, packet_types};
use uuid::Uuid;
use tokio::net::UdpSocket;
use std::net::SocketAddr;

fn validate_received_datagram(
	datagram: &[u8],
	current_seq: u32
) -> bool {
	// check magic bytes
	let magic_bytes = &datagram[0..3];
	if (magic_bytes != MAGIC_BYTES) {
		println!("Message not intended for us, discarding.");
		return false;
	}

	if (datagram.len() < 8){
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

	if (sequence_number != current_seq + 1){
		println!("Sequence number of received datagram was not an increment.");
		println!("Got {}, expected {}", sequence_number, current_seq + 1);
		return false;
	}    

	return true;
}

pub fn extract_payload(
	datagram: &[u8],
	current_seq: u32,
	uuid_param: &Uuid
) -> anyhow::Result<Vec<u8>> {
	// first check whether it is well-formatted for us or not.
	if (!validate_received_datagram(datagram, current_seq)){
		// return an error
		return Err(anyhow::anyhow!("Invalid datagram received"))
	}

	// is it a connection syn or ack? if so, then it won't have a uuid as a part of the payload
	let packet_type = datagram[3];
	let mut payloadStart: usize = 8;
	if (packet_type != packet_types::CONNECTION_SYN && 
		packet_type != packet_types::CONNECTION_ACK){
		
		// check the uuid
		let received_uuid = &datagram[8..24];
		let uuid_bytes = uuid_param.as_bytes();
		// println!("Expected UUID: {:02X?}", uuid_bytes);
		// println!("Received UUID: {:02X?}", received_uuid);
		if (uuid_bytes != received_uuid){
			return Err(anyhow::anyhow!("UUID does not match"))
		}

		payloadStart = 24;
	}

	// Extract the payload from the datagram
	let payload = &datagram[payloadStart..];
	return Ok(payload.to_vec())
}

pub fn validate_datagram_type(
    datagram: &[u8],
	current_seq: u32,
	packet_type: u8
) -> bool {
    let read_type = datagram[3];
    if (packet_type != read_type){
        println!("Packet type does not match");
        return false;
    }
    return validate_received_datagram(datagram, current_seq);
}


pub async fn send_datagram( 
    listener: &UdpSocket, 
    recipient: &SocketAddr, 
    packet_type: u8,
    sequence_number: &[u8],
    uuid_bytes: &[u8],
    payload: &[u8],
) -> anyhow::Result<()> {
    // we can build the datagram frame here.

    // println!("Building frame for {}", recipient);

    let mut frame: Vec<u8> = Vec::new();
    frame.extend_from_slice(&MAGIC_BYTES);
    frame.extend_from_slice(&[packet_type]);

    // println!("Building frame for {}", recipient);
    
    frame.extend_from_slice(&sequence_number);
    
    frame.extend_from_slice(uuid_bytes);

    frame.extend_from_slice(&payload);

    // println!("Sending frame {:02X?} for {}", frame, recipient);

    listener.send_to(&frame, recipient).await?;
    Ok(())
}
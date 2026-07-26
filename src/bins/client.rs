use rust_tcp::client::Client;
use std::io;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs8::DecodePublicKey};

use rand::thread_rng;
use uuid::Uuid;

use rust_tcp::constants::{MAGIC_BYTES, packet_types};
#[tokio::main]
async fn main() -> anyhow::Result<()> {

	let my_uuid = Uuid::new_v4();

	println!("Please enter the server address and port as <address>:<port>  >");
    let mut address_port = String::new();

    io::stdin().read_line(&mut address_port).expect("failed to readline");

	let split_address_port: Vec<&str> = address_port.trim().split(':').collect();

	let port = split_address_port[1].parse::<u16>().expect("failed to parse port");

	let address = split_address_port[0].to_string();

	println!("Please enter the server public key  >");
    let mut input_key = String::new();	
    io::stdin().read_line(&mut input_key).expect("failed to readline");

	let der = STANDARD.decode(input_key.trim())?;
	let public_key = RsaPublicKey::from_public_key_der(&der)?;



	let mut sequence_number: u32 = 0;



	let client = Client::connect(address, port).await?;

	sequence_number = send_connection_request(
		&client,
		&my_uuid,
		&public_key,
		sequence_number
	).await?;

	let mut buffer = [0u8; 264];
	loop {
		let (bytes_read, sender_addr) = client.receive_bytes(&mut buffer).await?;

		println!("Received datagram from {}: {} bytes", sender_addr, bytes_read);
	}


	// for _ in 0..100 {
	// 	client.send_message("Hello, world!").await?;
	// }

	Ok(())
}

async fn send_connection_request(
	client: &Client,
	uuid: &Uuid,
	public_key: &RsaPublicKey,
	mut sequence_number: u32
) -> anyhow::Result<u32> {

	// first, we create our UUID payload
	let data = uuid.to_string().into_bytes();
	let mut rng = thread_rng();
    let enc_data = public_key
        .encrypt(&mut rng, Pkcs1v15Encrypt, &data[..])
        .expect("Failed to encrypt data");

	// construct the frame of the message
	let encrypted_key_bytes = enc_data.clone(); // the actual payload

	let seq_bytes = sequence_number.to_be_bytes(); // 4 bytes

	// Combine them into one frame
	let mut frame: Vec<u8> = Vec::new();
	frame.extend_from_slice(&MAGIC_BYTES);	// 3 bytes
	frame.extend_from_slice(&[packet_types::CONNECTION_SYN]); // 1 byte
	frame.extend_from_slice(&seq_bytes);  // 4 bytes
	frame.extend_from_slice(&encrypted_key_bytes); // UUID payload 256 bytes

	// Send the combined bytes
	client.send_message_bytes(&frame).await?;

	println!("\n\nYour UUID is: {}", uuid.to_string());
	println!("Connecting to server... ");

	let mut buffer = [0u8; 264];
	let (bytes_read, sender_addr) = client.receive_bytes(&mut buffer).await?;

	// println!("Received {} bytes from {}", bytes_read, sender_addr);
	// println!("Received bytes: {:02X?}", &buffer[..bytes_read]);

	// let's inspect what was returned to check that the server acknowledged.

	let magic_bytes = &buffer[0..3];
	if (magic_bytes != MAGIC_BYTES) {
		println!("Message not intended for us, discarding.");
		return Err(anyhow::anyhow!("Invalid magic bytes received"));
	}

	let packet_type = buffer[3];
	if (packet_type != packet_types::CONNECTION_ACK) {
		println!("Expected connection ack, but received something else.");
		return Err(anyhow::anyhow!("Invalid packet type received"));
	}

	// overwrite
	sequence_number = u32::from_be_bytes(buffer[4..8].try_into()?);

	println!("Connected!");

	Ok(sequence_number + 1)
}
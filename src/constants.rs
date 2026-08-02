use uuid::Uuid;

pub mod packet_types {
	pub const CONNECTION_SYN: u8 = 0x00;
	pub const CONNECTION_ACK: u8 = 0x01;

    pub const MISC: u8 = 0x67;
}

pub static MAGIC_BYTES: [u8; 3] = [0x67, 0xf2, 0x5a];

pub struct ConnectionState {
    pub uuid: Uuid,
    pub sequence_number: u32,
}

pub static SERVER_HOST: &str = "127.0.0.1";
pub static SERVER_PORT: u16 = 8080;
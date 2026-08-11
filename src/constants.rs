use uuid::Uuid;

pub mod packet_types {
	pub const CONNECTION_SYN: u8 = 0x00;
	pub const CONNECTION_ACK: u8 = 0x01;

    pub const MISC: u8 = 0x67;

    pub const AUDIO_ANY: u8 = 0x10;
    pub const AUDIO_LOAD: u8 = 0x11;
    pub const AUDIO_SWAP: u8 = 0x12;
    pub const AUDIO_PLAY: u8 = 0x13;
    pub const AUDIO_PAUSE: u8 = 0x14;
    pub const AUDIO_FWD: u8 = 0x15;
    pub const AUDIO_BACK: u8 = 0x16;

    pub const LOADED_ACK: u8 = 0x20;
}

pub static MAGIC_BYTES: [u8; 3] = [0x67, 0xf2, 0x5a];

pub struct ConnectionState {
    pub uuid: Uuid,
    pub sequence_number: u32,
    pub acked_signal: bool,
}

pub static SERVER_HOST: &str = "127.0.0.1";
pub static SERVER_PORT: u16 = 8080;


// for audio file storage
pub static OUTPUT_FILE_PATH: &str = "temp-assets";
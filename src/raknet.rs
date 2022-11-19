use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bincode::{Decode, Encode};
use bincode::config::{BigEndian, Configuration, Fixint, SkipFixedArrayLength};
use bincode::de::Decoder;
use bincode::de::read::Reader;
use bincode::enc::Encoder;
use bincode::enc::write::Writer;
use bincode::error::{AllowedEnumVariants, DecodeError, EncodeError};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;

pub struct RakNetClient {
    pub start_time: Arc<SystemTime>,
    pub socket: Arc<UdpSocket>,
    pub rx: Receiver<(Duration, Packet)>
}

impl RakNetClient {
    pub async fn new() -> Self {
        let start_time = Arc::new(SystemTime::now());
        let socket = Arc::new(UdpSocket::bind("[::]:0").await.unwrap());
        let (tx, rx) = tokio::sync::mpsc::channel(1024);

        let handler_start_time = start_time.clone();
        let handler_socket = socket.clone();
        tokio::spawn(async move {
            let bincode_config: Configuration<BigEndian, Fixint, SkipFixedArrayLength> = Configuration::default();

            let mut buffer = [0; 1024];
            loop {
                let (buffer_size, _sender) = handler_socket.recv_from(&mut buffer).await.unwrap();
                let elapsed_time = handler_start_time.elapsed().unwrap();
                let packet: Packet = bincode::decode_from_slice(&buffer[..buffer_size], bincode_config).unwrap().0;
                tx.send((elapsed_time, packet)).await.expect("");
            }
        });

        Self { start_time, socket, rx }
    }
}

static UNCONNECTED_MAGIC: [u8; 16] = [0x00, 0xFF, 0xFF, 0x00, 0xFE, 0xFE, 0xFE, 0xFE, 0xFD, 0xFD, 0xFD, 0xFD, 0x12, 0x34, 0x56, 0x78];

#[derive(Debug)]
pub enum Packet {
    UnconnectedPing {
        elapsed_time: u64,
        /*unconnected_magic: [u8; 16],*/
        client_guid: u64
    },
    UnconnectedPong {
        elapsed_time: u64,
        server_guid: u64,
        /*unconnected_magic: [u8; 16],*/
        user_data: String
    }
}

impl Encode for Packet {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        match self {
            Packet::UnconnectedPing { elapsed_time, /*unconnected_magic, */client_guid } => {
                0x01u8.encode(encoder)?;
                elapsed_time.encode(encoder)?;
                UNCONNECTED_MAGIC.encode(encoder)?;
                client_guid.encode(encoder)
            }
            Packet::UnconnectedPong { elapsed_time, server_guid, /*unconnected_magic, */user_data } => {
                0x1Cu8.encode(encoder)?;
                elapsed_time.encode(encoder)?;
                server_guid.encode(encoder)?;
                UNCONNECTED_MAGIC.encode(encoder)?;
                {
                    let user_data_bytes = user_data.as_bytes();
                    (user_data_bytes.len() as u16).encode(encoder)?;
                    encoder.writer().write(user_data_bytes)
                }
            }
        }
    }
}

impl Decode for Packet {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let id = u8::decode(decoder)?;
        match id {
            0x01 => Ok(Self::UnconnectedPing {
                elapsed_time: {
                    let time: u64 = Decode::decode(decoder)?;
                    let _unconnected_magic: [u8; 16] = Decode::decode(decoder)?;
                    time
                },
                client_guid: Decode::decode(decoder)?
            }),
            0x1C => Ok(Self::UnconnectedPong {
                elapsed_time: Decode::decode(decoder)?,
                server_guid: {
                    let server_guid: u64 = Decode::decode(decoder)?;
                    let _unconnected_magic: [u8; 16] = Decode::decode(decoder)?;
                    server_guid
                },
                user_data: {
                    let user_data_length: u16 = Decode::decode(decoder)?;
                    let mut user_data = vec![0; user_data_length as usize];
                    decoder.reader().read(&mut user_data)?;
                    std::str::from_utf8(&user_data.to_owned()).unwrap()
                }.to_string()
            }),
            _ => Err(DecodeError::UnexpectedVariant {
                found: id as u32,
                allowed: &AllowedEnumVariants::Allowed(&[0x01, 0x1C]),
                type_name: core::any::type_name::<Packet>()
            })
        }
    }
}

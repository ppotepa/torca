use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};

pub fn protocol_health_check() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::var("TORCA_RELAY_HEALTH_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:8844".to_owned());
    let mut stream = TcpStream::connect_timeout(&address.parse()?, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(&RelayCodec::encode_request(&RelayRequest::Health)?)?;
    let mut header = [0_u8; RELAY_HEADER_LEN];
    stream.read_exact(&mut header)?;
    let frame_len = RelayCodec::frame_len_from_header(&header)?;
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&header);
    frame.resize(frame_len, 0);
    stream.read_exact(&mut frame[RELAY_HEADER_LEN..])?;
    match RelayCodec::decode_response(&frame)? {
        RelayResponse::Healthy => Ok(()),
        response => Err(format!("unexpected relay health response: {response:?}").into()),
    }
}

fn associated_data(context: PairingContextId) -> [u8; 16] {
    context.0.into_bytes()
}

fn encode_encrypted(payload: &EncryptedPairingPayload) -> Vec<u8> {
    let mut output = Vec::with_capacity(32 + 24 + 4 + payload.ciphertext.len());
    output.extend_from_slice(&payload.sender_public_key);
    output.extend_from_slice(&payload.nonce);
    let length = u32::try_from(payload.ciphertext.len()).unwrap_or(u32::MAX);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&payload.ciphertext);
    output
}

fn decode_encrypted(bytes: &[u8]) -> Result<EncryptedPairingPayload, PairingCoordinatorError> {
    if bytes.len() < 60 {
        return Err(PairingCoordinatorError::InvalidBlob);
    }
    let sender_public_key =
        bytes[0..32].try_into().map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    let nonce = bytes[32..56].try_into().map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    let length = u32::from_be_bytes(
        bytes[56..60].try_into().map_err(|_| PairingCoordinatorError::InvalidBlob)?,
    );
    let length = usize::try_from(length).map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    if bytes.len() != 60_usize.saturating_add(length) {
        return Err(PairingCoordinatorError::InvalidBlob);
    }
    Ok(EncryptedPairingPayload { sender_public_key, nonce, ciphertext: bytes[60..].to_vec() })
}

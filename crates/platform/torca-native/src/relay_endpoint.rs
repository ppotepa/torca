use crate::composition::NativeCompositionError;

const COMPILED_RELAY_ENDPOINT: &str = match option_env!("TORCA_RELAY_ENDPOINT") {
    Some(value) => value,
    None => "",
};

pub(crate) fn compiled_relay_endpoint() -> Result<(String, u16), NativeCompositionError> {
    parse_relay_endpoint(COMPILED_RELAY_ENDPOINT)
}

fn parse_relay_endpoint(value: &str) -> Result<(String, u16), NativeCompositionError> {
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| NativeCompositionError::new("relay endpoint must be host.onion:port"))?;
    let label = host.strip_suffix(".onion").ok_or_else(|| {
        NativeCompositionError::new("relay endpoint must use a v3 onion hostname")
    })?;
    if label.len() != 56 || !label.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7')) {
        return Err(NativeCompositionError::new(
            "relay endpoint contains an invalid v3 onion hostname",
        ));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| NativeCompositionError::new("relay endpoint contains an invalid port"))?;
    if port == 0 {
        return Err(NativeCompositionError::new("relay endpoint port must be non-zero"));
    }
    Ok((host.to_owned(), port))
}

#[cfg(test)]
mod tests {
    use super::parse_relay_endpoint;

    #[test]
    fn accepts_v3_onion_endpoint() {
        let host = format!("{}.onion", "a".repeat(56));
        assert_eq!(
            parse_relay_endpoint(&format!("{host}:443")).expect("valid endpoint"),
            (host, 443)
        );
    }

    #[test]
    fn rejects_clearnet_and_zero_port() {
        assert!(parse_relay_endpoint("example.com:443").is_err());
        let host = format!("{}.onion", "a".repeat(56));
        assert!(parse_relay_endpoint(&format!("{host}:0")).is_err());
    }
}

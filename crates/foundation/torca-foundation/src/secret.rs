use core::fmt;

/// Fixed-size sensitive byte storage with redacted diagnostics and wipe-on-drop.
///
/// This type intentionally does not implement `Clone` or `Copy`: creating a
/// second secret copy must be an explicit operation at the owning boundary.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SecretBytes<const N: usize>([u8; N]);

impl<const N: usize> SecretBytes<N> {
    pub const fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub const fn expose(&self) -> &[u8; N] {
        &self.0
    }
}

impl<const N: usize> fmt::Debug for SecretBytes<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretBytes([REDACTED])")
    }
}

impl<const N: usize> Drop for SecretBytes<N> {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::SecretBytes;

    #[test]
    fn debug_never_contains_secret_bytes() {
        let secret = SecretBytes::new([0x42_u8; 4]);
        let debug = format!("{secret:?}");
        assert!(!debug.contains("42"));
        assert!(debug.contains("REDACTED"));
    }
}

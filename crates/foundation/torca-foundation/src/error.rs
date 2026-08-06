use core::fmt;

/// Stable, non-sensitive machine-readable error code.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ErrorCode(&'static str);

impl ErrorCode {
    /// Creates a statically allocated error code.
    ///
    /// Codes must start and end with a lowercase ASCII letter or digit. Interior characters may
    /// additionally contain `.`, `_` or `-` separators.
    ///
    /// # Panics
    ///
    /// Panics when `value` is empty or does not follow the documented format. Constants using an
    /// invalid value fail during compilation.
    pub const fn new(value: &'static str) -> Self {
        assert!(is_valid_error_code(value), "invalid error code");
        Self(value)
    }

    /// Returns the machine-readable code.
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

const fn is_valid_error_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !is_code_alphanumeric(bytes[0]) {
        return false;
    }

    let mut index = 1;
    while index < bytes.len() {
        let byte = bytes[index];
        if !is_code_alphanumeric(byte) && byte != b'.' && byte != b'_' && byte != b'-' {
            return false;
        }
        index += 1;
    }

    is_code_alphanumeric(bytes[bytes.len() - 1])
}

const fn is_code_alphanumeric(value: u8) -> bool {
    matches!(value, b'a'..=b'z' | b'0'..=b'9')
}

/// Broad error category used by application boundaries without exposing implementation details.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ErrorCategory {
    /// Input failed validation.
    InvalidInput,
    /// Requested resource does not exist.
    NotFound,
    /// Current state conflicts with the requested operation.
    Conflict,
    /// Caller is not authenticated.
    Unauthorized,
    /// Caller is authenticated but not permitted.
    Forbidden,
    /// Dependency or transport is temporarily unavailable.
    Unavailable,
    /// Operation exceeded its deadline.
    Timeout,
    /// Operation was cancelled before completion.
    Cancelled,
    /// Unexpected implementation failure.
    Internal,
}

/// Retry guidance attached to a classified error.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RetryAdvice {
    /// Retrying the same request cannot succeed without changing input or state.
    Never,
    /// The operation may be retried immediately with the same idempotency identifier.
    Immediate,
    /// The operation may be retried after bounded backoff with the same idempotency identifier.
    Backoff,
}

/// Non-sensitive classification that can cross application and platform boundaries.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorDescriptor {
    code: ErrorCode,
    category: ErrorCategory,
    retry_advice: RetryAdvice,
}

impl ErrorDescriptor {
    /// Creates an error descriptor.
    pub const fn new(
        code: ErrorCode,
        category: ErrorCategory,
        retry_advice: RetryAdvice,
    ) -> Self {
        Self {
            code,
            category,
            retry_advice,
        }
    }

    /// Returns the stable error code.
    pub const fn code(self) -> ErrorCode {
        self.code
    }

    /// Returns the broad error category.
    pub const fn category(self) -> ErrorCategory {
        self.category
    }

    /// Returns retry guidance.
    pub const fn retry_advice(self) -> RetryAdvice {
        self.retry_advice
    }
}

/// Contract implemented by errors that expose a safe application-boundary classification.
pub trait ClassifiedError: std::error::Error {
    /// Returns a non-sensitive descriptor suitable for logging, bridge mapping and retry policy.
    fn descriptor(&self) -> ErrorDescriptor;
}

#[cfg(test)]
mod tests {
    use super::{ErrorCategory, ErrorCode, ErrorDescriptor, RetryAdvice};

    const STORAGE_UNAVAILABLE: ErrorCode = ErrorCode::new("storage.unavailable");

    #[test]
    fn descriptor_exposes_stable_machine_readable_classification() {
        let descriptor = ErrorDescriptor::new(
            STORAGE_UNAVAILABLE,
            ErrorCategory::Unavailable,
            RetryAdvice::Backoff,
        );

        assert_eq!(descriptor.code().as_str(), "storage.unavailable");
        assert_eq!(descriptor.category(), ErrorCategory::Unavailable);
        assert_eq!(descriptor.retry_advice(), RetryAdvice::Backoff);
    }
}

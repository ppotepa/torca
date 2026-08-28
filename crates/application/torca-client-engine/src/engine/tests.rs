#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_error_descriptors_are_stable() {
        let cases = [
            (EngineError::NotFound, "application.engine.not_found", ErrorCategory::NotFound, RetryAdvice::Never),
            (EngineError::Conflict, "application.engine.conflict", ErrorCategory::Conflict, RetryAdvice::Never),
            (EngineError::InvalidState, "application.engine.invalid_state", ErrorCategory::Conflict, RetryAdvice::Never),
            (EngineError::Repository, "application.engine.repository_unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff),
            (EngineError::Identity, "application.engine.identity_failed", ErrorCategory::Internal, RetryAdvice::Never),
            (EngineError::Pairing, "application.engine.pairing_failed", ErrorCategory::Conflict, RetryAdvice::Never),
            (EngineError::Messaging, "application.engine.messaging_failed", ErrorCategory::Conflict, RetryAdvice::Never),
            (EngineError::Unavailable, "application.engine.unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff),
        ];

        for (error, code, category, retry) in cases {
            let descriptor = error.descriptor();
            assert_eq!(descriptor.code().as_str(), code);
            assert_eq!(descriptor.category(), category);
            assert_eq!(descriptor.retry_advice(), retry);
        }
    }

}

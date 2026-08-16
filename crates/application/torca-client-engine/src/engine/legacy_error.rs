// Transitional source-compatibility shim for downstream repositories that
// still call `EngineError(String)`. The arbitrary detail is intentionally
// discarded; new code must construct a typed variant directly.
#[doc(hidden)]
#[allow(non_snake_case)]
pub fn EngineError(_legacy_detail: String) -> EngineError {
    EngineError::Repository
}

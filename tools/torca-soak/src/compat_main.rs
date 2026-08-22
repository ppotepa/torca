// Compatibility binary for historical battery-soak commands. Keeping the
// source in the same directory preserves module resolution while the actual
// implementation remains single-sourced in main.rs.
include!("main.rs");

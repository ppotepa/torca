use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let mut arguments = env::args().skip(1);
    let check = matches!(arguments.next().as_deref(), Some("--check"));
    let path = arguments.next().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("apps/client/flutter/lib/generated/torca_contract.dart"));
    let expected = torca_bridge::dart_contract_source();
    if check {
        let actual = fs::read_to_string(&path).unwrap_or_default();
        if actual != expected { eprintln!("generated contract is stale: {}", path.display()); std::process::exit(1); }
    } else {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent).expect("create output directory"); }
        fs::write(&path, expected).expect("write generated contract");
    }
}

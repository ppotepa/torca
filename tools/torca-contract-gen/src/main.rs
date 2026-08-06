use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let (check, path) = match arguments.as_slice() {
        [flag, path] if flag == "--check" => (true, PathBuf::from(path)),
        [path] => (false, PathBuf::from(path)),
        [] => (false, PathBuf::from("apps/client/flutter/lib/generated/torca_contract.dart")),
        _ => { eprintln!("usage: torca-contract-gen [--check] [output-path]"); std::process::exit(2); }
    };
    let expected = torca_bridge::dart_contract_source();
    if check { let actual = fs::read_to_string(&path).unwrap_or_default(); if actual != expected { eprintln!("generated contract is stale: {}", path.display()); std::process::exit(1); } } else { if let Some(parent) = path.parent() { fs::create_dir_all(parent).expect("create output directory"); } fs::write(&path, expected).expect("write generated contract"); }
}

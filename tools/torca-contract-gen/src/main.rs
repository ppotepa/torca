use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let schema_path = PathBuf::from("crates/platform/torca-contract/schema/torca_contract.json");
    let schema = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        eprintln!("missing canonical contract schema {}: {error}", schema_path.display());
        std::process::exit(1);
    });
    if !schema.contains("\"schema\": 1") || !schema.contains("\"profile.set\"") {
        eprintln!("canonical contract schema is invalid: {}", schema_path.display());
        std::process::exit(1);
    }
    let arguments: Vec<String> = env::args().skip(1).collect();
    let (check, path) = match arguments.as_slice() {
        [flag, path] if flag == "--check" => (true, PathBuf::from(path)),
        [path] => (false, PathBuf::from(path)),
        [] => (false, PathBuf::from("apps/client/flutter/lib/generated/torca_contract.dart")),
        _ => {
            eprintln!("usage: torca-contract-gen [--check] [output-path]");
            std::process::exit(2);
        }
    };
    let expected_path = PathBuf::from("crates/platform/torca-contract/schema/torca_contract.dart");
    let expected = fs::read_to_string(&expected_path).unwrap_or_else(|error| {
        eprintln!("missing contract projection {}: {error}", expected_path.display());
        std::process::exit(1);
    });
    if check {
        let actual = fs::read_to_string(&path).unwrap_or_default();
        if actual != expected {
            eprintln!("generated contract is stale: {}", path.display());
            std::process::exit(1);
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(&path, expected).expect("write generated contract");
    }
}

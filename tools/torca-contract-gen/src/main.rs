use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let schema_path = PathBuf::from("crates/platform/torca-contract/schema/torca_contract.json");
    let schema = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        eprintln!("missing canonical contract schema {}: {error}", schema_path.display());
        std::process::exit(1);
    });
    let schema_value: serde_json::Value = serde_json::from_str(&schema).unwrap_or_else(|error| {
        eprintln!("canonical contract schema is not valid JSON: {error}");
        std::process::exit(1);
    });
    let schema_version = schema_value.get("schema").and_then(serde_json::Value::as_u64);
    let contract_version = schema_value.get("contractVersion").and_then(serde_json::Value::as_u64);
    let commands = schema_value
        .pointer("/operations/commands")
        .and_then(serde_json::Value::as_array)
        .map(|values| values.iter().filter_map(serde_json::Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let queries = schema_value
        .pointer("/operations/queries")
        .and_then(serde_json::Value::as_array)
        .map(|values| values.iter().filter_map(serde_json::Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if schema_version != Some(1)
        || contract_version.is_none()
        || !commands.contains(&"profile.set")
        || !queries.contains(&"snapshot.get")
    {
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
    let template = fs::read_to_string(&expected_path).unwrap_or_else(|error| {
        eprintln!("missing contract projection {}: {error}", expected_path.display());
        std::process::exit(1);
    });
    let version_marker = format!("const int torcaContractVersion = {};", contract_version.unwrap());
    let expected = template
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("const int torcaContractVersion =") {
                version_marker.as_str()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let rust_path = PathBuf::from("crates/platform/torca-contract/src/generated_contract.rs");
    let rust_expected = format!(
        concat!(
            "// GENERATED FILE. DO NOT EDIT.\n",
            "// Generated from: crates/platform/torca-contract/schema/torca_contract.json\n\n",
            "pub const SCHEMA_VERSION: u16 = {schema_version};\n",
            "pub const CONTRACT_VERSION: u16 = {contract_version};\n",
            "pub const COMMANDS: &[&str] = &[{commands}];\n",
            "pub const QUERIES: &[&str] = &[{queries}];\n\n",
            "pub fn contains(kind: &str, name: &str) -> bool {{\n",
            "    match kind {{\n",
            "        \"command\" => COMMANDS.contains(&name),\n",
            "        \"query\" => QUERIES.contains(&name),\n",
            "        \"lifecycle\" => matches!(name, \"host_started\" | \"foregrounded\" | \"backgrounded\" | \"network_changed\" | \"low_memory\" | \"terminating\"),\n",
            "        _ => false,\n",
            "    }}\n",
            "}}\n",
        ),
        schema_version = schema_version.unwrap(),
        contract_version = contract_version.unwrap(),
        commands =
            commands.iter().map(|value| format!("\"{value}\"")).collect::<Vec<_>>().join(", "),
        queries = queries.iter().map(|value| format!("\"{value}\"")).collect::<Vec<_>>().join(", "),
    );
    if check {
        let actual = fs::read_to_string(&path).unwrap_or_default();
        if actual != expected {
            eprintln!("generated contract is stale: {}", path.display());
            std::process::exit(1);
        }
        let actual_rust = fs::read_to_string(&rust_path).unwrap_or_default();
        if actual_rust != rust_expected {
            eprintln!("generated Rust contract is stale: {}", rust_path.display());
            std::process::exit(1);
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(&path, expected).expect("write generated contract");
        fs::write(&rust_path, rust_expected).expect("write generated Rust contract");
    }
}

use std::env;
use std::fs;
use std::path::PathBuf;

const CONTRACT_VERSION_MARKER: &str = "__TORCA_CONTRACT_VERSION__";
const STORAGE_EPOCH_MARKER: &str = "__TORCA_STORAGE_EPOCH__";

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
    let release_path = PathBuf::from("release/version.json");
    let release_template = fs::read_to_string(&release_path).unwrap_or_else(|error| {
        eprintln!("missing release manifest {}: {error}", release_path.display());
        std::process::exit(1);
    });
    let release_value: serde_json::Value =
        serde_json::from_str(&release_template).unwrap_or_else(|error| {
            eprintln!("release manifest is not valid JSON: {error}");
            std::process::exit(1);
        });
    let storage_epoch = release_value
        .get("storageEpoch")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            eprintln!("release manifest storageEpoch is missing or invalid");
            std::process::exit(1);
        });
    let expected_path = PathBuf::from("crates/platform/torca-contract/schema/torca_contract.dart");
    let template = fs::read_to_string(&expected_path).unwrap_or_else(|error| {
        eprintln!("missing contract projection {}: {error}", expected_path.display());
        std::process::exit(1);
    });
    validate_dart_projection(&template).unwrap_or_else(|error| {
        eprintln!("canonical contract projection is incomplete: {error}");
        std::process::exit(1);
    });
    let expected = render_dart_template(&template, contract_version.unwrap(), storage_epoch)
        .unwrap_or_else(|error| {
            eprintln!("{error}: {}", expected_path.display());
            std::process::exit(1);
        });
    let rust_path = PathBuf::from("crates/platform/torca-contract/src/generated_contract.rs");
    let release_expected = render_release_manifest(&release_template, contract_version.unwrap())
        .unwrap_or_else(|error| {
            eprintln!("{error}: {}", release_path.display());
            std::process::exit(1);
        });
    let rust_expected = render_rust_contract(
        schema_version.unwrap(),
        contract_version.unwrap(),
        &commands,
        &queries,
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
        if release_template != release_expected {
            eprintln!("release manifest contractSchema is stale: {}", release_path.display());
            std::process::exit(1);
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(&path, expected).expect("write generated contract");
        fs::write(&rust_path, rust_expected).expect("write generated Rust contract");
        fs::write(&release_path, release_expected).expect("write release manifest contract schema");
    }
}

fn render_rust_contract(
    schema_version: u64,
    contract_version: u64,
    commands: &[&str],
    queries: &[&str],
) -> String {
    let commands =
        commands.iter().map(|value| format!("    \"{value}\",")).collect::<Vec<_>>().join("\n");
    let queries =
        queries.iter().map(|value| format!("    \"{value}\",")).collect::<Vec<_>>().join("\n");
    format!(
        concat!(
            "// GENERATED FILE. DO NOT EDIT.\n",
            "// Generated from: crates/platform/torca-contract/schema/torca_contract.json\n\n",
            "pub const SCHEMA_VERSION: u16 = {schema_version};\n",
            "pub const CONTRACT_VERSION: u16 = {contract_version};\n",
            "pub const COMMANDS: &[&str] = &[\n{commands}\n];\n",
            "pub const QUERIES: &[&str] = &[\n{queries}\n];\n\n",
            "pub fn contains(kind: &str, name: &str) -> bool {{\n",
            "    match kind {{\n",
            "        \"command\" => COMMANDS.contains(&name),\n",
            "        \"query\" => QUERIES.contains(&name),\n",
            "        \"lifecycle\" => matches!(\n",
            "            name,\n",
            "            \"host_started\"\n",
            "                | \"foregrounded\"\n",
            "                | \"backgrounded\"\n",
            "                | \"network_changed\"\n",
            "                | \"network_validated\"\n",
            "                | \"flutter_gateway_ready\"\n",
            "                | \"low_memory\"\n",
            "                | \"terminating\"\n",
            "        ),\n",
            "        _ => false,\n",
            "    }}\n",
            "}}\n",
        ),
        schema_version = schema_version,
        contract_version = contract_version,
        commands = commands,
        queries = queries,
    )
}

fn render_dart_template(
    template: &str,
    contract_version: u64,
    storage_epoch: u64,
) -> Result<String, String> {
    let marker_count = template.matches(CONTRACT_VERSION_MARKER).count();
    if marker_count != 1 {
        return Err(format!(
            "contract Dart template must contain exactly one {CONTRACT_VERSION_MARKER} marker; found {marker_count}"
        ));
    }
    let storage_marker_count = template.matches(STORAGE_EPOCH_MARKER).count();
    if storage_marker_count != 1 {
        return Err(format!(
            "contract Dart template must contain exactly one {STORAGE_EPOCH_MARKER} marker; found {storage_marker_count}"
        ));
    }
    Ok(template
        .replace(CONTRACT_VERSION_MARKER, &contract_version.to_string())
        .replace(STORAGE_EPOCH_MARKER, &storage_epoch.to_string()))
}

/// Keep the hand-authored projection from silently regressing to raw wire
/// strings. The generator owns this structural guard even though the enum
/// bodies remain in the canonical template for now; a missing typed surface
/// is a contract drift, not a harmless formatting change.
fn validate_dart_projection(template: &str) -> Result<(), String> {
    for marker in [
        "enum PairingRole",
        "enum PairingState",
        "enum ContactStatus",
        "enum MessageStatus",
        "enum AttachmentStatus",
        "typedState",
        "pairingStateFromWire",
        "attachmentStatusFromWire",
    ] {
        if !template.contains(marker) {
            return Err(format!("missing typed projection marker `{marker}`"));
        }
    }
    Ok(())
}

fn render_release_manifest(manifest: &str, contract_version: u64) -> Result<String, String> {
    let mut replacements = 0_u8;
    let mut rendered = manifest
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("\"contractSchema\"") {
                replacements = replacements.saturating_add(1);
                let indent = &line[..line.len() - line.trim_start().len()];
                format!("{indent}\"contractSchema\": {contract_version},")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    if replacements != 1 {
        return Err(format!(
            "release manifest must contain exactly one contractSchema field; found {replacements}"
        ));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_contract_version_from_the_only_marker() {
        assert_eq!(
            render_dart_template(
                "const v = __TORCA_CONTRACT_VERSION__; const e = __TORCA_STORAGE_EPOCH__;",
                17,
                3,
            ),
            Ok("const v = 17; const e = 3;".into())
        );
    }

    #[test]
    fn rejects_missing_or_duplicate_contract_version_markers() {
        assert!(render_dart_template("const v = 17;", 17, 3).is_err());
        assert!(
            render_dart_template(
                "__TORCA_CONTRACT_VERSION__ __TORCA_CONTRACT_VERSION__ __TORCA_STORAGE_EPOCH__",
                17,
                3,
            )
            .is_err()
        );
    }

    #[test]
    fn renders_release_contract_schema_without_reformatting_the_manifest() {
        let input = "{\n  \"version\": \"0.2\",\n  \"contractSchema\": 16,\n}\n";
        assert_eq!(
            render_release_manifest(input, 17),
            Ok("{\n  \"version\": \"0.2\",\n  \"contractSchema\": 17,\n}\n".into())
        );
    }
}

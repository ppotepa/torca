fn main() {
    println!("cargo:rerun-if-env-changed=TORCA_BUILD_ID");
    println!("cargo:rerun-if-env-changed=TORCA_PRODUCT_VERSION");
    println!("cargo:rerun-if-env-changed=TORCA_SOURCE_COMMIT");
    println!("cargo:rerun-if-env-changed=TORCA_SOURCE_FINGERPRINT");
    println!("cargo:rerun-if-env-changed=TORCA_RELAY_ENDPOINT");
    println!("cargo:rerun-if-env-changed=TORCA_RELAY_ENDPOINT_HASH");
}

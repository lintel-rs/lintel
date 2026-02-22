fn main() {
    let schema = lintel_config::schema();
    let json = serde_json::to_string_pretty(&schema).expect("schema serialization");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(format!("{out_dir}/lintel-config.schema.json"), json).expect("write schema");
    println!("cargo::rerun-if-changed=build.rs");
}

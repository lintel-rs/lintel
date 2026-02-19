fn main() {
    let schema = lintel_config::schema();
    let json = serde_json::to_string_pretty(&schema).expect("schema serialization");
    println!("{json}");
}

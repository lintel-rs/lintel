use std::path::Path;

use syntect::dumps::dump_binary;
use syntect::parsing::{SyntaxDefinition, SyntaxSetBuilder};

fn main() {
    println!("cargo::rerun-if-changed=syntaxes/");

    let syntaxes_dir = Path::new("syntaxes");
    let mut builder = SyntaxSetBuilder::new();
    builder.add_plain_text_syntax();

    for entry in std::fs::read_dir(syntaxes_dir).expect("failed to read syntaxes/ directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sublime-syntax") {
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let syntax = SyntaxDefinition::load_from_str(&src, true, None)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
            builder.add(syntax);
        }
    }

    let syntax_set = builder.build();
    let binary = dump_binary(&syntax_set);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("syntaxes.bin");
    std::fs::write(&out_path, binary)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_path.display()));
}

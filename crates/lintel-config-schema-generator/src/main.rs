use std::path::PathBuf;

use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, generate(cli))]
/// Generate JSON Schemas for Lintel configuration files
struct Cli {
    /// Output directory for generated schema files
    #[bpaf(positional("DIR"), fallback(PathBuf::from(".")))]
    output_dir: PathBuf,
}

fn main() {
    let cli = cli().run();

    std::fs::create_dir_all(&cli.output_dir).expect("failed to create output directory");

    let configs: &[(&str, serde_json::Value)] = &[
        ("lintel.json", lintel_config::schema()),
        (
            "lintel-catalog.json",
            lintel_catalog_builder::config::schema(),
        ),
    ];

    for (filename, schema) in configs {
        let path = cli.output_dir.join(filename);
        let json = serde_json::to_string_pretty(schema).expect("failed to serialize schema");
        std::fs::write(&path, format!("{json}\n")).expect("failed to write schema file");
        eprintln!("wrote {}", path.display());
    }
}

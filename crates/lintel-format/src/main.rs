use std::process::ExitCode;

use bpaf::Bpaf;

/// Format JSON, YAML, and TOML files
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
struct Cli {
    #[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)]
    global: lintel_cli_common::CliGlobalOptions,
    #[bpaf(external(lintel_format::format_args))]
    args: lintel_format::FormatArgs,
}

fn main() -> ExitCode {
    let cli = cli().run();

    match lintel_format::run(&cli.args, &cli.global) {
        Ok(had_unformatted) => {
            if had_unformatted {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)
        }
    }
}

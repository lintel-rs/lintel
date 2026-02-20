use std::process::ExitCode;

use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> ExitCode {
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_env("LINTEL_LOG") {
        tracing_subscriber::registry()
            .with(
                tracing_tree::HierarchicalLayer::new(2)
                    .with_targets(true)
                    .with_bracketed_fields(true)
                    .with_indent_lines(true)
                    .with_verbose_exit(true)
                    .with_verbose_entry(true)
                    .with_timer(tracing_tree::time::Uptime::default())
                    .with_writer(std::io::stderr),
            )
            .with(filter)
            .init();
    }

    let args: lintel_annotate::AnnotateArgs = bpaf::Parser::run(lintel_annotate::annotate_args());
    match lintel_annotate::run(&args, lintel_check::retriever::ReqwestClient::default()).await {
        Ok(result) => {
            for file in &result.annotated {
                println!("  {} ({})", file.path, file.schema_url);
            }
            for (path, err) in &result.errors {
                eprintln!("error: {path}: {err}");
            }
            let annotated = result.annotated.len();
            let skipped = result.skipped;
            let errors = result.errors.len();
            eprintln!("Annotated {annotated} files, skipped {skipped}, {errors} errors.");
            if errors > 0 {
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

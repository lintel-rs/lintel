use std::io::IsTerminal;
use std::time::Instant;

use ansi_term_codes::{BOLD, DIM, RESET};
use anyhow::Result;

use lintel_check::retriever::HttpClient;

/// Run the `annotate` command: add schema annotations to files.
pub async fn run<C: HttpClient>(
    args: &lintel_annotate::AnnotateArgs,
    client: C,
    verbose: bool,
) -> Result<bool> {
    let start = Instant::now();
    let result = lintel_annotate::run(args, client).await?;
    let had_errors = !result.errors.is_empty();

    if verbose {
        for file in &result.annotated {
            eprintln!("  + {} ({})", file.path, file.schema_url);
        }
        for file in &result.updated {
            eprintln!("  ~ {} ({})", file.path, file.schema_url);
        }
    }

    for (path, err) in &result.errors {
        eprintln!("error: {path}: {err}");
    }

    let annotated = result.annotated.len();
    let updated = result.updated.len();
    let skipped = result.skipped;
    let ms = start.elapsed().as_millis();

    if std::io::stderr().is_terminal() {
        if updated > 0 {
            eprintln!(
                "{BOLD}Annotated {annotated}, updated {updated} files{RESET} {DIM}(skipped {skipped}) in {ms}ms.{RESET}"
            );
        } else {
            eprintln!(
                "{BOLD}Annotated {annotated} files{RESET} {DIM}(skipped {skipped}) in {ms}ms.{RESET}"
            );
        }
    } else if updated > 0 {
        eprintln!("Annotated {annotated}, updated {updated} files (skipped {skipped}) in {ms}ms.");
    } else {
        eprintln!("Annotated {annotated} files (skipped {skipped}) in {ms}ms.");
    }

    Ok(had_errors)
}

use std::io::IsTerminal;
use std::time::Instant;

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
                "\x1b[1mAnnotated {annotated}, updated {updated} files\x1b[0m \x1b[2m(skipped {skipped}) in {ms}ms.\x1b[0m"
            );
        } else {
            eprintln!(
                "\x1b[1mAnnotated {annotated} files\x1b[0m \x1b[2m(skipped {skipped}) in {ms}ms.\x1b[0m"
            );
        }
    } else if updated > 0 {
        eprintln!("Annotated {annotated}, updated {updated} files (skipped {skipped}) in {ms}ms.");
    } else {
        eprintln!("Annotated {annotated} files (skipped {skipped}) in {ms}ms.");
    }

    Ok(had_errors)
}

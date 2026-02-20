use std::io::IsTerminal;
use std::time::Instant;

use anyhow::Result;

use lintel_check::retriever::HttpClient;

use crate::ValidateArgs;

/// Run the `annotate` command: add schema annotations to files.
pub async fn run<C: HttpClient>(
    args: &mut ValidateArgs,
    client: C,
    verbose: bool,
    update: bool,
) -> Result<bool> {
    super::merge_config(args);

    let annotate_args = lintel_annotate::AnnotateArgs {
        exclude: args.exclude.clone(),
        cache_dir: args.cache_dir.clone(),
        no_catalog: args.no_catalog,
        format: args.format.map(|f| {
            match f {
                crate::FileFormat::Json => "json",
                crate::FileFormat::Json5 => "json5",
                crate::FileFormat::Jsonc => "jsonc",
                crate::FileFormat::Toml => "toml",
                crate::FileFormat::Yaml => "yaml",
                crate::FileFormat::Markdown => "markdown",
            }
            .to_string()
        }),
        schema_cache_ttl: args.schema_cache_ttl.clone(),
        update,
        globs: args.globs.clone(),
    };

    let start = Instant::now();
    let result = lintel_annotate::run(&annotate_args, client).await?;
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

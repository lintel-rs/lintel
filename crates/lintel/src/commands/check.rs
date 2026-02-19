use std::io::IsTerminal;
use std::time::Instant;

use anyhow::Result;
use miette::Report;

use lintel_check::retriever::HttpClient;
use lintel_check::validate;

use crate::ValidateArgs;

use super::merge_config;

/// Run the `check` command: fancy miette output with colors and timing.
pub async fn run<C: HttpClient>(args: &mut ValidateArgs, client: C, verbose: bool) -> Result<bool> {
    merge_config(args);

    let lib_args = validate::ValidateArgs::from(&*args);
    let start = Instant::now();
    let result = validate::run_with(&lib_args, client, |file| {
        if verbose {
            eprintln!("{}", super::format_checked_verbose(file));
        }
    })
    .await?;
    let had_errors = result.has_errors();
    let n = result.files_checked();

    for error in result.errors {
        eprintln!("{:?}", Report::new_boxed(error.into_diagnostic()));
    }

    let ms = start.elapsed().as_millis();
    if std::io::stderr().is_terminal() {
        eprintln!("\x1b[1mChecked {n} files\x1b[0m \x1b[2min {ms}ms.\x1b[0m");
    } else {
        eprintln!("Checked {n} files in {ms}ms.");
    }

    Ok(had_errors)
}

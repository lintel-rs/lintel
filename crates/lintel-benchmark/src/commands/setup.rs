use anyhow::Result;

use crate::tools;

#[allow(clippy::unnecessary_wraps)]
pub fn run() -> Result<()> {
    println!("Installing validator tools for comparison...");
    println!();

    for tool in tools::all_tools() {
        if tool.is_available() {
            println!("  {}: already installed ({})", tool.name(), tool.version());
        } else {
            tools::install(tool.as_ref());
        }
    }

    println!();
    println!("Done. Run `cargo run --release --package lintel-benchmark -- run` to benchmark.");
    Ok(())
}

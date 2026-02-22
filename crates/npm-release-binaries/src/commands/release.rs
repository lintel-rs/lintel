use super::generate;

pub fn run(opts: &generate::Options<'_>) -> miette::Result<()> {
    generate::run(opts)?;
    super::publish::run(opts.pkg_config, opts.output_dir, false)?;
    Ok(())
}

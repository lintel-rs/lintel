use super::generate;
use super::publish;

pub fn run(
    opts: &generate::Options<'_>,
    publish_opts: &publish::Options<'_>,
) -> miette::Result<()> {
    generate::run(opts)?;
    super::publish::run(opts.pkg_config, opts.output_dir, publish_opts)?;
    Ok(())
}

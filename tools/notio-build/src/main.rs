//! The `notio-build` binary: parse the command line, run the build.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = notio_build::Cli::parse();
    notio_build::run(&cli)?;
    Ok(())
}

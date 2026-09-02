//! The `ferroterm-build` binary: parse the command line, run the build.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = ferroterm_build::Cli::parse();
    ferroterm_build::run(&cli)?;
    Ok(())
}

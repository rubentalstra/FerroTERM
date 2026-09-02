//! The `ferroterm-build` binary: parse the command line, run the build.
#![expect(clippy::print_stdout, reason = "a command-line tool reports to stdout")]

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = ferroterm_build::Cli::parse();
    let report = ferroterm_build::run(&cli)?;
    println!(
        "{}: {} concepts, {} designations, {} is-a edges, {} words, written to {}",
        report.version_uri,
        report.concepts,
        report.designations,
        report.is_a_edges,
        report.words,
        cli.out.display()
    );
    Ok(())
}

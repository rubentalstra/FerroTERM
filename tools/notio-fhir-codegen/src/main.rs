//! The `notio-fhir-codegen` binary: parse the command line, run the generator.
#![expect(clippy::print_stdout, reason = "a command-line tool reports to stdout")]

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = notio_fhir_codegen::Cli::parse();
    let report = notio_fhir_codegen::run(&cli)?;
    println!(
        "notio-fhir-codegen: {} types across {} files",
        report.types,
        report.files.len()
    );
    Ok(())
}

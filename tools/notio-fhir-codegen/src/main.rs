//! The `notio-fhir-codegen` binary: parse the command line, run the generator.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = notio_fhir_codegen::Cli::parse();
    notio_fhir_codegen::run(&cli)?;
    Ok(())
}

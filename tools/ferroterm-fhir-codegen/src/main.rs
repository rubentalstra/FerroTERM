//! The `ferroterm-fhir-codegen` binary: parse the command line, run the generator.
#![expect(clippy::print_stdout, reason = "a command-line tool reports to stdout")]

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = ferroterm_fhir_codegen::Cli::parse();
    let report = ferroterm_fhir_codegen::run(&cli)?;
    for (module, types) in &report.types {
        println!("ferroterm-fhir-codegen: {module}: {types} types");
    }
    println!("ferroterm-fhir-codegen: {} files", report.files.len());
    Ok(())
}

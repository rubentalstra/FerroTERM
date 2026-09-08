//! The `fhir-codegen` binary: parse the command line, run the generator.
#![expect(clippy::print_stdout, reason = "a command-line tool reports to stdout")]

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = fhir_codegen::Cli::parse();
    match fhir_codegen::run(&cli)? {
        fhir_codegen::Report::Emit(report) => {
            for (module, types) in &report.types {
                println!("fhir-codegen: {module}: {types} types");
            }
            println!("fhir-codegen: {} files", report.files.len());
        }
        fhir_codegen::Report::Terminology(report) => {
            for (module, counts) in &report.counts {
                println!(
                    "fhir-codegen: {module}: {} code systems, {} value sets, {} without a status, {} with a duplicate code",
                    counts.code_systems,
                    counts.value_sets,
                    counts.without_status,
                    counts.with_duplicate_code
                );
            }
        }
    }
    Ok(())
}

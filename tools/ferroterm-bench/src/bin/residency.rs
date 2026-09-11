//! What one structure of a built artifact costs in memory, measured on its own.
//!
//! A served SNOMED edition holds more memory than its side files appear to
//! need, and a total says nothing about where it goes. This binary loads
//! exactly one structure and reports the resident memory that loading it cost,
//! so each figure is measured rather than inferred from a total (#322).
//!
//! One structure per process, because two structures in one process cannot be
//! told apart: the allocator does not give a structure back its own arena, and
//! a second load reuses what the first freed. The caller runs this once per
//! structure and the reader adds them up.
//!
//! The resident figure comes from `ps`, which is how every other measurement
//! in this repository reads it; a process cannot read its own resident set
//! without `unsafe` or a platform crate, and the workspace forbids `unsafe`.
#![allow(
    clippy::print_stdout,
    reason = "a measurement binary reports on stdout, which is what its caller reads"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use clap::Parser;
use serde::Serialize;

/// How many resident readings one measurement takes.
///
/// An odd count, so the median is a reading rather than an average of two
/// (`ferroterm-bench`'s own sampler, #304).
const SAMPLES: usize = 5;

/// How long between two readings.
const INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

#[derive(Parser)]
#[command(
    about = "The resident memory one structure of a built artifact costs, measured on its own"
)]
struct Cli {
    /// The artifact directory, the one a server is pointed at.
    #[arg(long)]
    artifact: PathBuf,
    /// Which structure to load, or `baseline` to load none.
    #[arg(long)]
    structure: Structure,
    /// Loads every structure up to and including this one, in the order the
    /// provider loads them, rather than this one alone.
    ///
    /// Two peaks of two processes do not add up, because they do not happen at
    /// the same moment. Cumulative peaks do: the difference between loading
    /// through one structure and through the one before it is what that
    /// structure costs with everything before it already resident.
    #[arg(long)]
    cumulative: bool,
}

/// The structures a built artifact holds, each in its own file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Structure {
    /// Nothing: the cost of the process itself, which every other figure is
    /// measured against.
    Baseline,
    /// `hierarchy.bin`: the CSR adjacency and the roaring transitive closure.
    Hierarchy,
    /// `hierarchy.bin`, and the child adjacency the provider transposes from
    /// it at open. Its cost is this figure less the hierarchy's.
    Children,
    /// `text.bin`: the `fst` designation dictionary and its postings.
    Text,
    /// `members.bin`: the reference set tables the ECL evaluator reads.
    Members,
    /// `attributes.bin`: the attribute adjacency and its inverted index.
    Attributes,
    /// `refsets.bin`: the reference set membership bitmaps.
    ///
    /// The manifest names this one `refsets` and the tables `members`, which is
    /// the other way round from what the types are called.
    Refsets,
    /// `identifiers.bin`: the alternate identifier table.
    Identifiers,
    /// `store.redb`: the concept and designation store, opened not read.
    Store,
    /// Everything a served edition loads, through the provider that serves it.
    Edition,
}

impl Structure {
    /// The file this structure is read from, when it is one file.
    fn file(self) -> Option<&'static str> {
        match self {
            Self::Baseline | Self::Edition => None,
            Self::Hierarchy | Self::Children => Some("hierarchy.bin"),
            Self::Text => Some("text.bin"),
            Self::Members => Some("members.bin"),
            Self::Attributes => Some("attributes.bin"),
            Self::Refsets => Some("refsets.bin"),
            Self::Identifiers => Some("identifiers.bin"),
            Self::Store => Some("store.redb"),
        }
    }
}

/// The order the provider loads the structures in, for the cumulative mode.
const LOAD_ORDER: [Structure; 7] = [
    Structure::Store,
    Structure::Hierarchy,
    Structure::Children,
    Structure::Text,
    Structure::Refsets,
    Structure::Attributes,
    Structure::Members,
];

/// One measurement, as the caller reads it.
#[derive(Serialize)]
struct Measurement {
    /// The structure measured.
    structure: Structure,
    /// The artifact it was read from.
    artifact: String,
    /// The size of the file on disk, where the structure is one file.
    serialized_bytes: Option<u64>,
    /// The resident memory of this process after loading it.
    resident_bytes: Option<u64>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if !cli.artifact.join("manifest.json").is_file() {
        bail!("{} holds no manifest.json", cli.artifact.display());
    }
    // Every structure is held until the reading is taken, because a value the
    // compiler can see is dead is a value the allocator may already have back.
    let held = if cli.cumulative {
        let mut held = Vec::new();
        for structure in LOAD_ORDER {
            held.push(load(&cli.artifact, structure)?);
            if structure == cli.structure {
                break;
            }
        }
        held
    } else {
        vec![load(&cli.artifact, cli.structure)?]
    };
    let measurement = Measurement {
        structure: cli.structure,
        artifact: cli.artifact.display().to_string(),
        serialized_bytes: cli
            .structure
            .file()
            .and_then(|name| std::fs::metadata(cli.artifact.join(name)).ok())
            .map(|meta| meta.len()),
        resident_bytes: resident(),
    };
    println!("{}", serde_json::to_string(&measurement)?);
    drop(held);
    Ok(())
}

/// Whatever was loaded, kept alive until the reading is taken.
///
/// Nothing reads a variant's value, which is the point: the value exists to
/// hold the memory it allocated until the resident reading is taken.
#[expect(
    dead_code,
    reason = "each variant holds a structure alive for the measurement and is never read"
)]
enum Held {
    /// Nothing, for the baseline.
    Nothing,
    /// One structure, as the type that owns its memory.
    Hierarchy(Box<concept_graph::persist::Hierarchy>),
    Children(Box<(concept_graph::persist::Hierarchy, concept_graph::csr::Csr)>),
    Text(Box<designation_index::index::TextIndex>),
    Members(Box<concept_graph::refsets::RefsetMembers>),
    Attributes(Box<concept_graph::attributes::Attributes>),
    Refsets(Box<concept_graph::members::Memberships>),
    Identifiers(Box<concept_graph::identifiers::Identifiers>),
    Store(Box<concept_store::store::Store>),
    Edition(Box<fhir_terminology::snomed::SnomedProvider>),
}

/// Reads one structure of the artifact at `dir`.
fn load(dir: &Path, structure: Structure) -> anyhow::Result<Held> {
    let open = |name: &str| -> anyhow::Result<std::io::BufReader<std::fs::File>> {
        let path = dir.join(name);
        let file = std::fs::File::open(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Ok(std::io::BufReader::new(file))
    };
    Ok(match structure {
        Structure::Baseline => Held::Nothing,
        Structure::Hierarchy => Held::Hierarchy(Box::new(
            concept_graph::persist::Hierarchy::read_from(&mut open("hierarchy.bin")?)
                .context("cannot read the hierarchy")?,
        )),
        Structure::Children => {
            let hierarchy =
                concept_graph::persist::Hierarchy::read_from(&mut open("hierarchy.bin")?)
                    .context("cannot read the hierarchy")?;
            let children = hierarchy
                .is_a
                .transpose()
                .context("cannot transpose the hierarchy")?;
            Held::Children(Box::new((hierarchy, children)))
        }
        Structure::Text => Held::Text(Box::new(
            designation_index::persist::read_from(&mut open("text.bin")?)
                .context("cannot read the text index")?,
        )),
        Structure::Members => Held::Members(Box::new(
            concept_graph::refsets::RefsetMembers::read_from(&mut open("members.bin")?)
                .context("cannot read the reference set tables")?,
        )),
        Structure::Attributes => Held::Attributes(Box::new(
            concept_graph::attributes::Attributes::read_from(&mut open("attributes.bin")?)
                .context("cannot read the attributes")?,
        )),
        Structure::Refsets => Held::Refsets(Box::new(
            concept_graph::members::Memberships::read_from(&mut open("refsets.bin")?)
                .context("cannot read the memberships")?,
        )),
        Structure::Identifiers => Held::Identifiers(Box::new(
            concept_graph::identifiers::Identifiers::read_from(&mut open("identifiers.bin")?)
                .context("cannot read the identifiers")?,
        )),
        Structure::Store => Held::Store(Box::new(
            concept_store::store::Store::open(&dir.join("store.redb"))
                .context("cannot open the store")?,
        )),
        Structure::Edition => Held::Edition(Box::new(
            fhir_terminology::snomed::SnomedProvider::open(dir, "en")
                .context("cannot open the edition")?,
        )),
    })
}

/// The resident memory of this process, as the median of several readings.
fn resident() -> Option<u64> {
    let mut samples: Vec<u64> = Vec::with_capacity(SAMPLES);
    for sample in 0..SAMPLES {
        if sample > 0 {
            std::thread::sleep(INTERVAL);
        }
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        let kilobytes = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .ok()?;
        samples.push(kilobytes * 1024);
    }
    samples.sort_unstable();
    samples.get(SAMPLES.div_euclid(2)).copied()
}

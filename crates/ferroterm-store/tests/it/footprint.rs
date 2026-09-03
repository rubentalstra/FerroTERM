//! Prints where the bytes of a built artifact go, table by table, for the
//! footprint work. Ignored by default: it reads the local artifact under
//! `artifacts/nl/` (a licensed edition, never committed).

use std::path::PathBuf;

use ferroterm_store::tables;
use redb::{
    ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    TableHandle,
};

fn local_artifact() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/nl/store.redb");
    path.exists().then_some(path)
}

fn report<K: redb::Key + 'static, V: redb::Value + 'static>(
    txn: &redb::ReadTransaction,
    def: TableDefinition<'_, K, V>,
) {
    let table = txn.open_table(def).expect("table opens");
    let stats = table.stats().expect("stats");
    let rows = table.len().expect("len");
    println!(
        "{:<18} rows {:>9}  stored {:>7} MiB  metadata {:>6} MiB  fragmented {:>6} MiB  leaves {:>8}  height {}",
        def.name(),
        rows,
        stats.stored_bytes() / 1_048_576,
        stats.metadata_bytes() / 1_048_576,
        stats.fragmented_bytes() / 1_048_576,
        stats.leaf_pages(),
        stats.tree_height(),
    );
}

#[test]
#[ignore = "needs the local NL artifact under artifacts/nl/"]
fn the_local_artifact_footprint_by_table() {
    let Some(path) = local_artifact() else {
        panic!("no artifact under artifacts/nl/");
    };
    let size = std::fs::metadata(&path).expect("metadata").len();
    println!("file {} MiB", size / 1_048_576);
    let db = ReadOnlyDatabase::open(&path).expect("opens");
    let txn = db.begin_read().expect("read txn");
    report(&txn, tables::META);
    report(&txn, tables::CODES);
    report(&txn, tables::CONCEPTS);
    report(&txn, tables::DESIGNATIONS);
    report(&txn, tables::ACCEPTABILITY);
    report(&txn, tables::PREFERRED);
    report(&txn, tables::PROPERTIES);
    report(&txn, tables::PROPERTY_KEYS);
    report(&txn, tables::DESIGNATION_USES);
    report(&txn, tables::LANGUAGE_REFSETS);
    report(&txn, tables::ACCEPTABILITIES);
    report(&txn, tables::BLOBS);
}

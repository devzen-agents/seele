use serde_json::Value;
use std::process::Command;
use tempfile::TempDir;
const BIN: &str = env!("CARGO_BIN_EXE_seele");
fn run(db: &std::path::Path, args: &[&str]) -> (bool, Value) {
    let o = Command::new(BIN)
        .arg("--db")
        .arg(db)
        .args(["--fake-embedder", "--json"])
        .args(args)
        .output()
        .unwrap();
    (
        o.status.success(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn missing_source_backup_creates_neither_source_nor_destination() {
    let t = TempDir::new().unwrap();
    let source = t.path().join("missing.db");
    let dest = t.path().join("new-dir/backup.db");
    let (ok, v) = run(&source, &["backup", dest.to_str().unwrap()]);
    assert!(!ok);
    assert_eq!(v["ok"], false);
    assert!(!source.exists());
    assert!(!dest.parent().unwrap().exists());
}
#[test]
fn guarded_reads_and_writes_refuse_missing_database() {
    let t = TempDir::new().unwrap();
    let db = t.path().join("absent/记忆.db");
    for args in [
        vec!["--require-existing-db", "list"],
        vec!["--require-existing-db", "save", "title", "content"],
    ] {
        assert!(!run(&db, &args).0);
        assert!(!db.parent().unwrap().exists());
    }
}
#[test]
fn creation_and_existing_unicode_space_paths_work() {
    let t = TempDir::new().unwrap();
    let db = t.path().join("记忆 with spaces.db");
    let (ok, s) = run(&db, &["save", "synthetic", "content"]);
    assert!(ok);
    let id = s["data"]["id"].as_str().unwrap();
    let (ok, s) = run(&db, &["--require-existing-db", "show", id]);
    assert!(ok);
    assert_eq!(s["data"]["content"], "content");
    let backup = t.path().join("backup.db");
    assert!(
        run(
            &db,
            &["--require-existing-db", "backup", backup.to_str().unwrap()]
        )
        .0
    );
    assert!(run(&backup, &["--require-existing-db", "show", id]).0);
    assert!(!run(&db, &["backup", backup.to_str().unwrap()]).0);
}
#[test]
fn directory_is_not_a_database() {
    let t = TempDir::new().unwrap();
    assert!(!run(t.path(), &["--require-existing-db", "list"]).0);
    assert!(
        !run(
            t.path(),
            &["backup", t.path().join("out.db").to_str().unwrap()]
        )
        .0
    );
    assert!(!t.path().join("out.db").exists());
}

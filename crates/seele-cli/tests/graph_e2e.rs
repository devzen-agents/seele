//! E2E for `seele graph export` (ADR-17): the real binary writes
//! `graphify-out/graph.json`, hubs toggle, and the optional graphify
//! hand-off degrades to a warning when absent and to an error when it
//! runs and fails.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

const SEELE_BIN: &str = env!("CARGO_BIN_EXE_seele");

fn run(args: &[&str]) -> (String, String, std::process::ExitStatus) {
    let out = Command::new(SEELE_BIN)
        .env("SEELE_FAKE_EMBEDDER", "1")
        // A stray GRAPHIFY_BIN on the developer machine must not leak in.
        .env_remove("GRAPHIFY_BIN")
        .args(args)
        .output()
        .expect("spawn");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status,
    )
}

fn save(db: &str, title: &str, project: &str, ty: &str) -> String {
    let (stdout, _, status) = run(&[
        "--db",
        db,
        "--json",
        "save",
        title,
        "content",
        "--project",
        project,
        "--type",
        ty,
    ]);
    assert!(status.success(), "save failed: {stdout}");
    let v: Value = serde_json::from_str(&stdout).expect("save json");
    v["data"]["id"].as_str().expect("save id").to_string()
}

fn export(db: &str, out: &Path, extra: &[&str]) -> (Value, String, std::process::ExitStatus) {
    let out_s = out.to_str().unwrap();
    let mut args = vec![
        "--db",
        db,
        "--json",
        "graph",
        "export",
        "--project",
        "p",
        "--out",
        out_s,
    ];
    args.extend_from_slice(extra);
    let (stdout, stderr, status) = run(&args);
    let v: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("export did not print a JSON envelope ({e}): stdout={stdout} stderr={stderr}")
    });
    (v, stderr, status)
}

fn read_graph(out: &Path) -> Value {
    let path = out.join("graphify-out").join("graph.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("graph.json missing at {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("graph.json is JSON")
}

/// A stand-in `graphify` that exits with `code`. Batch file on Windows
/// (std spawns `.cmd` through cmd.exe), shell script elsewhere.
fn fake_graphify(dir: &Path, code: i32) -> PathBuf {
    #[cfg(windows)]
    {
        let p = dir.join(format!("graphify-exit{code}.cmd"));
        std::fs::write(
            &p,
            format!("@echo off\r\necho fake graphify\r\nexit /b {code}\r\n"),
        )
        .unwrap();
        p
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(format!("graphify-exit{code}"));
        std::fs::write(&p, format!("#!/bin/sh\necho fake graphify\nexit {code}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }
}

#[test]
fn export_writes_graph_json_with_hubs_and_links() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let a = save(db, "alpha", "p", "decision");
    let b = save(db, "beta", "p", "bugfix");
    let (stdout, _, status) = run(&["--db", db, "--json", "link", &a, &b, "derives_from"]);
    assert!(status.success(), "link failed: {stdout}");
    // Another project's observation must stay out of a --project export.
    save(db, "other", "q", "memory");

    let out = td.path().join("out");
    let (v, _, status) = export(db, &out, &[]);
    assert!(status.success(), "{v}");
    assert_eq!(v["ok"], true);
    let d = &v["data"];
    assert_eq!(d["observations"], 2, "{d}");
    assert_eq!(d["hubs"], 3, "project p + types decision/bugfix: {d}");
    assert_eq!(d["nodes"], 5);
    assert_eq!(d["links"], 5, "1 link + 2 hub edges per observation: {d}");
    assert_eq!(d["skipped_dangling"], 0);
    assert_eq!(d["graphify"]["status"], "not_requested");

    let g = read_graph(&out);
    assert_eq!(g["directed"], true);
    assert_eq!(g["nodes"].as_array().unwrap().len(), 5);
    let edges = g["links"].as_array().unwrap();
    assert_eq!(edges.len(), 5);
    let real = edges
        .iter()
        .find(|e| e["edge_kind"] == "link")
        .expect("real link");
    assert_eq!(
        real["source"],
        format!("seele_obs_{}", a.to_ascii_lowercase())
    );
    assert_eq!(
        real["target"],
        format!("seele_obs_{}", b.to_ascii_lowercase())
    );
    assert_eq!(real["relation"], "derives_from");
    assert_eq!(real["confidence"], "EXTRACTED");
    let titles: Vec<&str> = g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["file_type"] == "observation")
        .map(|n| n["label"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"alpha") && titles.contains(&"beta") && !titles.contains(&"other"));
}

#[test]
fn export_no_hubs_is_observations_and_links_only() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    let a = save(db, "alpha", "p", "decision");
    let b = save(db, "beta", "p", "decision");
    run(&["--db", db, "link", &a, &b, "related_to"]);

    let out = td.path().join("out");
    let (v, _, status) = export(db, &out, &["--no-hubs", "--content-chars", "0"]);
    assert!(status.success(), "{v}");
    assert_eq!(v["data"]["hubs"], 0);
    assert_eq!(v["data"]["nodes"], 2);
    assert_eq!(v["data"]["links"], 1);
    let g = read_graph(&out);
    assert!(
        g["nodes"][0].get("description").is_none(),
        "content omitted"
    );
}

#[test]
fn run_graphify_missing_is_a_warning_not_an_error() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save(db, "alpha", "p", "decision");

    let out = td.path().join("out");
    let missing = td.path().join("nope.exe");
    let (v, stderr, status) = export(
        db,
        &out,
        &[
            "--run-graphify",
            "--graphify-bin",
            missing.to_str().unwrap(),
        ],
    );
    assert!(status.success(), "absence must not fail: {v}");
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["graphify"]["status"], "not_found");
    assert!(v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w == "graphify-not-found"));
    assert!(
        stderr.contains("graphify not found"),
        "human hint on stderr: {stderr}"
    );
    assert!(
        out.join("graphify-out").join("graph.json").is_file(),
        "graph.json still written"
    );
}

#[test]
fn run_graphify_success_reports_ran() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save(db, "alpha", "p", "decision");
    let fake = fake_graphify(td.path(), 0);

    let out = td.path().join("out");
    let (v, stderr, status) = export(
        db,
        &out,
        &[
            "--run-graphify",
            "--graphify-bin",
            fake.to_str().unwrap(),
            "--no-viz",
        ],
    );
    assert!(status.success(), "{v} {stderr}");
    assert_eq!(v["data"]["graphify"]["status"], "ran");
    assert_eq!(v["data"]["graphify"]["source"], "--graphify-bin");
    assert!(
        stderr.contains("fake graphify"),
        "graphify output forwarded to stderr: {stderr}"
    );
}

#[test]
fn run_graphify_failure_is_an_error_but_graph_json_stays() {
    let td = TempDir::new().unwrap();
    let db = td.path().join("s.db");
    let db = db.to_str().unwrap();
    save(db, "alpha", "p", "decision");
    let fake = fake_graphify(td.path(), 3);

    let out = td.path().join("out");
    let (v, _, status) = export(
        db,
        &out,
        &["--run-graphify", "--graphify-bin", fake.to_str().unwrap()],
    );
    assert!(
        !status.success(),
        "explicitly requested graphify failed: {v}"
    );
    assert_eq!(v["ok"], false);
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("graphify cluster-only failed"),
        "{v}"
    );
    assert!(
        out.join("graphify-out").join("graph.json").is_file(),
        "graph.json still written"
    );
}

//! `seele graph export` — the observation graph as a Graphify
//! `graph.json` (NetworkX node-link JSON): observations are nodes,
//! typed `links` and pending/judged `memory_relations` are edges, plus
//! optional project/type hub nodes so community detection has
//! structure to work with on sparse memories.
//!
//! Graphify (<https://github.com/Graphify-Labs/graphify>) is OPTIONAL
//! and never a dependency (ADR-17): the JSON is written
//! unconditionally, and `--run-graphify` hands it to the `graphify` CLI
//! only when one is found (`--graphify-bin` > `$GRAPHIFY_BIN` > PATH).
//! A missing graphify is an in-band warning (`graphify-not-found`),
//! never an error — the same degradation contract as the ONNX → Fake
//! embedder fallback. A graphify that was found but failed IS an
//! error: the caller asked for it explicitly.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args as ClapArgs, Subcommand};
use seele_http::dto::{LinkDto, ListRequest, ObservationDto, RelationDto, RelationListQuery};
use seele_storage::LinkQuery;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::app::OutputOpts;
use crate::output;

/// Env var naming the graphify executable. Resolution order:
/// `--graphify-bin` > `$GRAPHIFY_BIN` > PATH lookup.
pub const GRAPHIFY_BIN_ENV: &str = "GRAPHIFY_BIN";
/// In-band warning code: `--run-graphify` was asked but no executable
/// was found. The export itself succeeded.
pub const WARN_GRAPHIFY_NOT_FOUND: &str = "graphify-not-found";
/// Directory graphify expects its artefacts in (`<out>/graphify-out/`).
pub const GRAPHIFY_OUT_DIR: &str = "graphify-out";
/// File name graphify reads and rewrites.
pub const GRAPH_JSON: &str = "graph.json";

#[derive(Subcommand, Debug)]
pub enum GraphCmd {
    /// Write the memory graph as Graphify's `graph.json`
    /// (`<out>/graphify-out/graph.json`); optionally run graphify on it.
    Export(ExportArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ExportArgs {
    /// Only this project's observations (default: every project).
    #[arg(long)]
    pub project: Option<String>,
    /// Directory that receives `graphify-out/graph.json`. Defaults to
    /// the current directory.
    #[arg(long, default_value = ".")]
    pub out: PathBuf,
    /// Skip the project/type hub nodes: observations + links/relations only.
    #[arg(long)]
    pub no_hubs: bool,
    /// Include soft-deleted observations.
    #[arg(long)]
    pub include_deleted: bool,
    /// Characters of each observation's content copied into the node
    /// `description` (0 omits the content entirely).
    #[arg(long, default_value_t = 400)]
    pub content_chars: usize,
    /// After writing, run `graphify cluster-only --no-label` on the
    /// output when a graphify CLI is found (`--graphify-bin` >
    /// `$GRAPHIFY_BIN` > PATH). Not finding one is a warning, not an
    /// error; a graphify that runs and fails is an error.
    #[arg(long)]
    pub run_graphify: bool,
    /// Explicit path to the graphify executable (overrides
    /// `$GRAPHIFY_BIN` and PATH).
    #[arg(long, value_name = "PATH")]
    pub graphify_bin: Option<PathBuf>,
    /// Pass `--no-viz` to graphify (skip `graph.html`).
    #[arg(long)]
    pub no_viz: bool,
}

/// `data` payload of the `--json` envelope.
#[derive(Debug, Serialize)]
pub struct ExportReport {
    /// Absolute path of the written `graph.json`.
    pub graph_json: String,
    #[serde(flatten)]
    pub counts: Counts,
    pub graphify: GraphifyStatus,
}

/// Node/edge counters of one export.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub observations: usize,
    pub hubs: usize,
    pub nodes: usize,
    pub links: usize,
    /// Links/relations whose endpoint is not in the export (other
    /// project, soft-deleted, or missing).
    pub skipped_dangling: usize,
    /// Relations with `orphaned`/`ignored` judgment: never exported.
    pub skipped_relations: usize,
}

/// What happened with the optional graphify hand-off.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GraphifyStatus {
    NotRequested,
    NotFound { hint: String },
    Ran { bin: String, source: String },
}

pub async fn run(
    cmd: GraphCmd,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    match cmd {
        GraphCmd::Export(args) => run_export(args, db, fake_embedder, out).await,
    }
}

async fn run_export(
    args: ExportArgs,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let svc = crate::app::build_service(db, fake_embedder)?;
    let observations = svc.list_observations(ListRequest {
        project: args.project.clone(),
        scope: None,
        r#type: None,
        topic_key: None,
        session_id: None,
        limit: None,
        include_deleted: args.include_deleted,
    })?;
    let links: Vec<LinkDto> = svc
        .links
        .list(LinkQuery::default())?
        .into_iter()
        .map(LinkDto::from)
        .collect();
    let relations = svc.list_relations(RelationListQuery {
        source_id: None,
        target_id: None,
        relation: None,
        status: None,
        limit: None,
    })?;

    let opts = BuildOpts {
        hubs: !args.no_hubs,
        content_chars: args.content_chars,
        project: args.project.clone(),
        exported_at: now_iso8601(),
    };
    let (graph, counts) = build_graph(&observations, &links, &relations, &opts);

    let out_dir = args.out.join(GRAPHIFY_OUT_DIR);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", out_dir.display()))?;
    let graph_path = out_dir.join(GRAPH_JSON);
    std::fs::write(&graph_path, serde_json::to_vec_pretty(&graph)?)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", graph_path.display()))?;
    let graph_path = std::path::absolute(&graph_path).unwrap_or(graph_path);

    let graphify = if args.run_graphify {
        let resolved = resolve_graphify_bin(
            args.graphify_bin.as_deref(),
            std::env::var_os(GRAPHIFY_BIN_ENV).as_deref(),
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("PATHEXT").as_deref(),
            cfg!(windows),
        );
        match resolved {
            Ok((bin, source)) => {
                run_graphify(&bin, &args.out, &graph_path, args.no_viz)?;
                GraphifyStatus::Ran {
                    bin: bin.display().to_string(),
                    source: source.as_str().to_string(),
                }
            }
            Err(tried) => {
                let hint = not_found_hint(&tried, &args.out, &graph_path);
                // stderr prose for humans; the envelope warning for agents.
                eprintln!("seele: warning — {hint}");
                output::push_warning(WARN_GRAPHIFY_NOT_FOUND);
                GraphifyStatus::NotFound { hint }
            }
        }
    } else {
        GraphifyStatus::NotRequested
    };

    let report = ExportReport {
        graph_json: graph_path.display().to_string(),
        counts,
        graphify,
    };
    output::emit_split(&report, || human_summary(&report), out.json)
}

fn human_summary(r: &ExportReport) -> String {
    let c = &r.counts;
    let mut lines = vec![
        format!("graph.json written: {}", r.graph_json),
        format!(
            "  nodes: {} ({} observations + {} hubs), links: {}, skipped: {} dangling, {} unjudged-relations",
            c.nodes, c.observations, c.hubs, c.links, c.skipped_dangling, c.skipped_relations
        ),
    ];
    lines.push(match &r.graphify {
        GraphifyStatus::NotRequested => {
            "  graphify: not requested (pass --run-graphify to cluster + report)".to_string()
        }
        GraphifyStatus::NotFound { hint } => format!("  graphify: not found — {hint}"),
        GraphifyStatus::Ran { bin, source } => format!("  graphify: ran {bin} (from {source})"),
    });
    lines.join("\n")
}

// ---------------------------------------------------------------- graph building

/// Knobs of [`build_graph`], kept separate from clap so the builder is
/// unit-testable without a CLI parse.
pub struct BuildOpts {
    pub hubs: bool,
    pub content_chars: usize,
    pub project: Option<String>,
    pub exported_at: String,
}

/// Build the node-link document. Pure: no I/O, deterministic for a
/// given input order.
pub fn build_graph(
    observations: &[ObservationDto],
    links: &[LinkDto],
    relations: &[RelationDto],
    opts: &BuildOpts,
) -> (Value, Counts) {
    let mut counts = Counts::default();
    let mut nodes: Vec<Value> = Vec::with_capacity(observations.len());
    let mut edges: Vec<Value> = Vec::new();
    // SEELE id (as the DTOs spell it) → node id, plus the node's
    // source_file so edges can carry it (graphify backfills it anyway,
    // but explicit is cheaper for its validator).
    let mut node_of: HashMap<String, String> = HashMap::with_capacity(observations.len());
    let mut source_file_of: HashMap<String, String> = HashMap::with_capacity(observations.len());
    // Hubs keyed by node id so each project/type appears once; BTreeMap
    // keeps the output order stable.
    let mut hubs: BTreeMap<String, Value> = BTreeMap::new();

    for o in observations {
        let nid = obs_node_id(&o.id);
        let sf = source_file_for(o);
        let mut node = Map::new();
        node.insert("id".into(), json!(nid));
        node.insert("label".into(), json!(o.title));
        node.insert("source_file".into(), json!(sf));
        node.insert("file_type".into(), json!("observation"));
        node.insert("_origin".into(), json!("seele"));
        node.insert("seele_id".into(), json!(o.id));
        node.insert("seele_type".into(), json!(o.r#type));
        node.insert("project".into(), json!(o.project));
        node.insert("scope".into(), json!(o.scope));
        node.insert("topic_key".into(), json!(o.topic_key));
        node.insert(
            "created_at".into(),
            json!(iso8601_from_millis(o.created_at)),
        );
        if o.deleted_at.is_some() {
            node.insert("deleted".into(), json!(true));
        }
        if opts.content_chars > 0 {
            node.insert(
                "description".into(),
                json!(truncate_chars(&o.content, opts.content_chars)),
            );
        }
        if opts.hubs {
            let project_label = o.project.as_deref().unwrap_or("(no project)");
            let project_hub = hub_node("project", project_label, &sf);
            let type_hub = hub_node("type", &o.r#type, &sf);
            for (hub, relation) in [(project_hub, "in_project"), (type_hub, "has_type")] {
                let hub_id = hub["id"].as_str().unwrap_or_default().to_string();
                edges.push(json!({
                    "source": nid,
                    "target": hub_id,
                    "relation": relation,
                    "confidence": "EXTRACTED",
                    "confidence_score": 1.0,
                    "weight": 0.5,
                    "edge_kind": "hub",
                    "source_file": sf,
                }));
                hubs.entry(hub_id).or_insert(hub);
            }
        }
        node_of.insert(o.id.clone(), nid.clone());
        source_file_of.insert(nid.clone(), sf);
        nodes.push(Value::Object(node));
    }
    counts.observations = observations.len();

    for l in links {
        match (node_of.get(&l.from_id), node_of.get(&l.to_id)) {
            (Some(from), Some(to)) => edges.push(json!({
                "source": from,
                "target": to,
                "relation": l.link_type,
                "confidence": "EXTRACTED",
                "confidence_score": 1.0,
                "weight": 1.0,
                "edge_kind": "link",
                "seele_link_id": l.id,
                "source_file": source_file_of.get(from),
            })),
            _ => counts.skipped_dangling += 1,
        }
    }

    for r in relations {
        let (confidence, default_score) = match r.judgment_status {
            "judged" => ("EXTRACTED", 1.0),
            "pending" => ("INFERRED", 0.5),
            _ => {
                counts.skipped_relations += 1;
                continue;
            }
        };
        match (node_of.get(&r.source_id), node_of.get(&r.target_id)) {
            (Some(from), Some(to)) => edges.push(json!({
                "source": from,
                "target": to,
                "relation": r.relation,
                "confidence": confidence,
                "confidence_score": r.confidence.unwrap_or(default_score),
                "weight": 1.0,
                "edge_kind": "relation",
                "judgment_status": r.judgment_status,
                "seele_relation_id": r.id,
                "source_file": source_file_of.get(from),
            })),
            _ => counts.skipped_dangling += 1,
        }
    }

    counts.hubs = hubs.len();
    nodes.extend(hubs.into_values());
    counts.nodes = nodes.len();
    counts.links = edges.len();

    let graph = json!({
        "directed": true,
        "multigraph": false,
        "graph": {
            "generator": "seele",
            "generator_version": env!("CARGO_PKG_VERSION"),
            "format": "graphify-node-link",
            "project": opts.project,
            "exported_at": opts.exported_at,
            "hyperedges": [],
        },
        "nodes": nodes,
        "links": edges,
        "hyperedges": [],
    });
    (graph, counts)
}

fn hub_node(kind: &str, name: &str, source_file: &str) -> Value {
    json!({
        "id": format!("seele_{kind}_{}", slug(name)),
        "label": format!("{kind}: {name}"),
        "source_file": source_file,
        "file_type": "hub",
        "_origin": "seele",
        "hub": kind,
    })
}

/// Node id for an observation: lowercase ULID under a stable prefix.
pub fn obs_node_id(seele_id: &str) -> String {
    format!("seele_obs_{}", seele_id.to_ascii_lowercase())
}

/// Pseudo-URI graphify can show as the node's "file": `seele://<project>/<id>`.
fn source_file_for(o: &ObservationDto) -> String {
    format!("seele://{}/{}", o.project.as_deref().unwrap_or("_"), o.id)
}

/// Lowercase `[a-z0-9_]` identifier fragment; never empty.
pub fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_sep = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    let trimmed = out.trim_end_matches('_');
    if trimmed.is_empty() {
        "x".to_string()
    } else {
        trimmed.to_string()
    }
}

/// First `n` chars (char-boundary safe) with an ellipsis when cut.
fn truncate_chars(s: &str, n: usize) -> String {
    let mut it = s.chars();
    let head: String = it.by_ref().take(n).collect();
    if it.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

// ---------------------------------------------------------------- time

/// `YYYY-MM-DDTHH:MM:SSZ` from unix milliseconds. Hand-rolled (civil
/// date from days, H. Hinnant) so the CLI crate does not pick up a date
/// dependency for one string.
pub fn iso8601_from_millis(ms: i64) -> String {
    iso8601_from_secs(ms.div_euclid(1000))
}

fn iso8601_from_secs(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso8601_from_secs(secs)
}

// ---------------------------------------------------------------- graphify hand-off

/// Where the executable came from (reported in the envelope so an agent
/// can tell a project-local override from a global install).
#[derive(Debug, PartialEq, Eq)]
pub enum BinSource {
    Flag,
    Env,
    Path,
}

impl BinSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Flag => "--graphify-bin",
            Self::Env => GRAPHIFY_BIN_ENV,
            Self::Path => "PATH",
        }
    }
}

/// Resolve the graphify executable: `flag` > `env_bin` > `path_var`
/// scan. The first source that is *configured* decides — an explicit
/// flag or env var pointing at a missing file is reported as not found
/// instead of silently falling through to another binary. `Err` lists
/// what was tried, for the hint.
pub fn resolve_graphify_bin(
    flag: Option<&Path>,
    env_bin: Option<&OsStr>,
    path_var: Option<&OsStr>,
    pathext: Option<&OsStr>,
    windows: bool,
) -> Result<(PathBuf, BinSource), Vec<String>> {
    if let Some(p) = flag {
        return if p.is_file() {
            Ok((p.to_path_buf(), BinSource::Flag))
        } else {
            Err(vec![format!("--graphify-bin {} (not a file)", p.display())])
        };
    }
    if let Some(e) = env_bin.filter(|e| !e.is_empty()) {
        let p = PathBuf::from(e);
        return if p.is_file() {
            Ok((p, BinSource::Env))
        } else {
            Err(vec![format!(
                "{GRAPHIFY_BIN_ENV}={} (not a file)",
                p.display()
            )])
        };
    }
    let mut tried = Vec::new();
    if let Some(path_var) = path_var {
        for dir in std::env::split_paths(path_var) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            for cand in candidates(&dir, windows, pathext) {
                if cand.is_file() {
                    return Ok((cand, BinSource::Path));
                }
            }
        }
        tried.push("PATH lookup for `graphify`".to_string());
    } else {
        tried.push("PATH (unset)".to_string());
    }
    Err(tried)
}

fn candidates(dir: &Path, windows: bool, pathext: Option<&OsStr>) -> Vec<PathBuf> {
    if !windows {
        return vec![dir.join("graphify")];
    }
    let exts: Vec<String> = pathext
        .and_then(|p| p.to_str())
        .map(|s| {
            s.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| e.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec![".exe".into(), ".cmd".into(), ".bat".into(), ".com".into()]);
    let mut v: Vec<PathBuf> = exts
        .iter()
        .map(|e| dir.join(format!("graphify{e}")))
        .collect();
    v.push(dir.join("graphify"));
    v
}

fn not_found_hint(tried: &[String], out: &Path, graph_path: &Path) -> String {
    format!(
        "graphify not found (tried: {}). graph.json is ready at {}. Install graphify \
         (`uv tool install graphifyy` or `pipx install graphifyy`), or point {GRAPHIFY_BIN_ENV} \
         / --graphify-bin at the executable, then run: graphify cluster-only {} --graph {} --no-label",
        tried.join("; "),
        graph_path.display(),
        out.display(),
        graph_path.display()
    )
}

/// Arguments handed to graphify. `out` is the directory that contains
/// `graphify-out/`; graphify writes its report/labels/html beside the
/// `--graph` file when that file lives inside a `graphify-out/` dir.
pub fn graphify_args(out: &Path, graph_path: &Path, no_viz: bool) -> Vec<String> {
    let mut v = vec![
        "cluster-only".to_string(),
        out.display().to_string(),
        "--graph".to_string(),
        graph_path.display().to_string(),
        "--no-label".to_string(),
    ];
    if no_viz {
        v.push("--no-viz".to_string());
    }
    v
}

fn run_graphify(bin: &Path, out: &Path, graph_path: &Path, no_viz: bool) -> anyhow::Result<()> {
    let output = Command::new(bin)
        .args(graphify_args(out, graph_path, no_viz))
        .output()
        .map_err(|e| anyhow::anyhow!("failed to spawn graphify at {}: {e}", bin.display()))?;
    // graphify's own chatter goes to stderr: stdout stays reserved for
    // the JSON envelope.
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        anyhow::bail!(
            "graphify cluster-only failed ({}); graph.json is still at {}",
            output.status,
            graph_path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str, title: &str, project: Option<&str>, ty: &str) -> ObservationDto {
        ObservationDto {
            id: id.to_string(),
            session_id: None,
            r#type: ty.to_string(),
            title: title.to_string(),
            content: "body ".repeat(200),
            tool_name: None,
            project: project.map(str::to_string),
            scope: "project".to_string(),
            topic_key: Some(format!("{ty}/x")),
            revision_count: 0,
            duplicate_count: 0,
            last_seen_at: 1_700_000_000_000,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
            deleted_at: None,
            metadata: Value::Null,
        }
    }

    fn link(from: &str, to: &str, ty: &str) -> LinkDto {
        LinkDto {
            id: "01LINK".to_string(),
            from_id: from.to_string(),
            to_id: to.to_string(),
            link_type: ty.to_string(),
            metadata: Value::Null,
            created_at: 0,
        }
    }

    fn relation(from: &str, to: &str, status: &'static str) -> RelationDto {
        RelationDto {
            id: "01REL".to_string(),
            sync_id: "s".to_string(),
            source_id: from.to_string(),
            target_id: to.to_string(),
            relation: "supersedes",
            judgment_status: status,
            reason: None,
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
            created_at: 0,
        }
    }

    fn opts(hubs: bool) -> BuildOpts {
        BuildOpts {
            hubs,
            content_chars: 10,
            project: None,
            exported_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn slug_is_lowercase_identifier_and_never_empty() {
        assert_eq!(slug("Community OS"), "community_os");
        assert_eq!(slug("  decisión/ABC-1  "), "decisión_abc_1");
        assert_eq!(slug("///"), "x");
        assert_eq!(
            obs_node_id("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            "seele_obs_01arz3ndektsv4rrffq69g5fav"
        );
    }

    #[test]
    fn build_graph_with_hubs_links_and_dangling_filter() {
        let a = obs("01A", "alpha", Some("p"), "decision");
        let b = obs("01B", "beta", Some("p"), "bugfix");
        let links = vec![
            link("01A", "01B", "derives_from"),
            link("01A", "01ZZZ", "related_to"),
        ];
        let (g, c) = build_graph(&[a, b], &links, &[], &opts(true));
        // 2 observations + hubs: project p, type decision, type bugfix.
        assert_eq!(c.observations, 2);
        assert_eq!(c.hubs, 3);
        assert_eq!(c.nodes, 5);
        // 1 real link + 2 hub edges per observation; the dangling one is skipped.
        assert_eq!(c.links, 1 + 4);
        assert_eq!(c.skipped_dangling, 1);
        assert_eq!(g["directed"], true);
        assert_eq!(g["graph"]["generator"], "seele");
        let nodes = g["nodes"].as_array().unwrap();
        let first = &nodes[0];
        assert_eq!(first["id"], "seele_obs_01a");
        assert_eq!(first["source_file"], "seele://p/01A");
        assert_eq!(first["created_at"], "2023-11-14T22:13:20Z");
        assert_eq!(first["description"], "body body …");
        let real = g["links"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["edge_kind"] == "link")
            .unwrap();
        assert_eq!(real["source"], "seele_obs_01a");
        assert_eq!(real["target"], "seele_obs_01b");
        assert_eq!(real["relation"], "derives_from");
        assert_eq!(real["confidence"], "EXTRACTED");
    }

    #[test]
    fn build_graph_without_hubs_and_without_content() {
        let a = obs("01A", "alpha", None, "memory");
        let mut o = opts(false);
        o.content_chars = 0;
        let (g, c) = build_graph(&[a], &[], &[], &o);
        assert_eq!(c.hubs, 0);
        assert_eq!(c.nodes, 1);
        assert_eq!(c.links, 0);
        let node = &g["nodes"][0];
        assert!(node.get("description").is_none());
        assert_eq!(node["source_file"], "seele://_/01A");
    }

    #[test]
    fn relations_map_judgment_to_confidence() {
        let a = obs("01A", "a", Some("p"), "decision");
        let b = obs("01B", "b", Some("p"), "decision");
        let rels = vec![
            relation("01A", "01B", "judged"),
            relation("01B", "01A", "pending"),
            relation("01A", "01B", "ignored"),
            relation("01A", "01B", "orphaned"),
        ];
        let (g, c) = build_graph(&[a, b], &[], &rels, &opts(false));
        assert_eq!(c.links, 2);
        assert_eq!(c.skipped_relations, 2);
        let edges = g["links"].as_array().unwrap();
        assert_eq!(edges[0]["confidence"], "EXTRACTED");
        assert_eq!(edges[0]["confidence_score"], 1.0);
        assert_eq!(edges[1]["confidence"], "INFERRED");
        assert_eq!(edges[1]["confidence_score"], 0.5);
    }

    #[test]
    fn iso8601_known_instants() {
        assert_eq!(iso8601_from_millis(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso8601_from_millis(1_700_000_000_000),
            "2023-11-14T22:13:20Z"
        );
        assert_eq!(iso8601_from_millis(951_782_400_000), "2000-02-29T00:00:00Z");
        assert_eq!(iso8601_from_millis(-1_000), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn resolve_prefers_flag_then_env_then_path_and_never_falls_through() {
        let td = tempfile::TempDir::new().unwrap();
        let flag_bin = td.path().join("flag-graphify.exe");
        let env_bin = td.path().join("env-graphify.exe");
        let path_dir = td.path().join("bin");
        std::fs::create_dir_all(&path_dir).unwrap();
        let path_bin = path_dir.join("graphify.cmd");
        for p in [&flag_bin, &env_bin, &path_bin] {
            std::fs::write(p, "").unwrap();
        }
        let pathext = OsStr::new(".EXE;.CMD");
        let path_var = std::env::join_paths([&path_dir]).unwrap();

        let path_var = path_var.as_os_str();

        let (bin, src) = resolve_graphify_bin(
            Some(&flag_bin),
            Some(env_bin.as_os_str()),
            Some(path_var),
            Some(pathext),
            true,
        )
        .unwrap();
        assert_eq!((bin, src), (flag_bin.clone(), BinSource::Flag));

        let (bin, src) = resolve_graphify_bin(
            None,
            Some(env_bin.as_os_str()),
            Some(path_var),
            Some(pathext),
            true,
        )
        .unwrap();
        assert_eq!((bin, src), (env_bin.clone(), BinSource::Env));

        let (bin, src) =
            resolve_graphify_bin(None, None, Some(path_var), Some(pathext), true).unwrap();
        assert_eq!((bin, src), (path_bin.clone(), BinSource::Path));

        // Configured-but-missing never falls through to the next source.
        let missing = td.path().join("nope.exe");
        let err = resolve_graphify_bin(Some(&missing), None, Some(path_var), Some(pathext), true)
            .unwrap_err();
        assert!(err[0].contains("--graphify-bin"), "{err:?}");
        let err = resolve_graphify_bin(
            None,
            Some(missing.as_os_str()),
            Some(path_var),
            Some(pathext),
            true,
        )
        .unwrap_err();
        assert!(err[0].contains(GRAPHIFY_BIN_ENV), "{err:?}");

        // Unix candidates: bare name only, so the .cmd is not found.
        assert!(resolve_graphify_bin(None, None, Some(path_var), None, false).is_err());
        assert!(resolve_graphify_bin(None, None, None, None, false).is_err());
    }

    #[test]
    fn graphify_args_shape() {
        let args = graphify_args(
            Path::new("out"),
            Path::new("out/graphify-out/graph.json"),
            true,
        );
        assert_eq!(args[0], "cluster-only");
        assert_eq!(args[1], "out");
        assert_eq!(args[2], "--graph");
        assert!(args[3].ends_with("graph.json"));
        assert!(args.contains(&"--no-label".to_string()));
        assert!(args.contains(&"--no-viz".to_string()));
        assert!(
            !graphify_args(Path::new("o"), Path::new("g"), false).contains(&"--no-viz".to_string())
        );
    }
}

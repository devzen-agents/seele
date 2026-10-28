# Graphify hand-off (optional)

`seele graph export` writes the memory graph in the file format
[Graphify](https://github.com/Graphify-Labs/graphify) reads, so a memory
can be clustered, reported on, rendered and queried with Graphify's own
tools — and merged with the code graph of a repository. Graphify is
**optional**: SEELE never imports it, never needs it installed, and the
export is complete without it. See ADR-17
(`docs/plans/arquitectura/17-graphify-opcional-no-critico.md`).

## What gets written

```
<out>/graphify-out/graph.json      # NetworkX node-link JSON, directed
```

| Element | Source in SEELE | Node/edge attributes |
|---|---|---|
| node `observation` | each observation (soft-deleted ones only with `--include-deleted`) | `id` = `seele_obs_<ulid>`, `label` = title, `source_file` = `seele://<project>/<ULID>`, `seele_type`, `project`, `scope`, `topic_key`, `created_at`, `description` (first `--content-chars` characters, default 400; `0` omits) |
| node `hub` | one per project and one per observation type (skip with `--no-hubs`) | `id` = `seele_project_<slug>` / `seele_type_<slug>` |
| edge `link` | `links` table | `relation` = link type, `confidence: EXTRACTED` |
| edge `relation` | `memory_relations` with status `judged` (`EXTRACTED`) or `pending` (`INFERRED`); `orphaned`/`ignored` are skipped | `judgment_status`, `confidence_score` = the relation's confidence |
| edge `hub` | observation → its project / type hub | `in_project`, `has_type`, weight 0.5 |

Hubs exist because a memory is usually sparse in explicit links; without
them community detection returns one singleton per observation. Edges
whose endpoint is outside the export (another project, deleted) are
counted as `skipped_dangling` and dropped.

Privacy: the file contains memory content (`<private>…</private>` spans
were already stripped at save time). Keep it where you keep the database.

## Running graphify on it

```bash
seele graph export --project my-app --out . --run-graphify        # cluster + GRAPH_REPORT.md + graph.html
seele graph export --project my-app --out . --run-graphify --no-viz
graphify cluster-only . --graph graphify-out/graph.json --no-label  # the same, by hand
graphify-mcp graphify-out/graph.json                                # query it from an agent (MCP stdio)
graphify merge-graphs graphify-out/graph.json ../my-app/graphify-out/graph.json --out merged.json
```

`--run-graphify` locates the executable in this order and the **first
configured source decides** (a flag or env var pointing at a missing
file is reported, not skipped):

1. `--graphify-bin <path>`
2. `$GRAPHIFY_BIN`
3. `PATH` (`graphify`, plus `PATHEXT` variants on Windows: `.exe`, `.cmd`, …)

| Situation | Exit | Envelope (`--json`) |
|---|---|---|
| not requested | 0 | `data.graphify.status = "not_requested"` |
| requested, not found | 0 | `status = "not_found"`, `warnings: ["graphify-not-found"]`, hint on stderr |
| requested, ran OK | 0 | `status = "ran"`, `bin`, `source` (`--graphify-bin` / `GRAPHIFY_BIN` / `PATH`) |
| requested, ran and failed | 1 | `ok: false`, `error` names graphify; `graph.json` stays on disk |

Absence is a degradation, not a failure — the same contract as the ONNX
embedder falling back to `FakeEmbedder` with the `fake-embedder-fallback`
warning. Graphify's own output is forwarded to stderr so stdout stays a
clean JSON envelope. `--no-label` is always passed: community naming is
the one Graphify step that calls an LLM, and SEELE's core never pays for
one; run `graphify label <out>` yourself if you want named communities.

## Install graphify (if you want the hand-off)

```bash
uv tool install graphifyy        # or: pipx install graphifyy
# or point at an existing install:
export GRAPHIFY_BIN=/path/to/venv/bin/graphify
```

Graphify is Python ≥ 3.10; its code extractors are local (tree-sitter)
and its clustering is local (no LLM with `--no-label`).

# Táctica — Graphify opcional y referencias portables (tareas atómicas)

**Estado**: Propuesta · 2026-10-22 · **ADR**: `docs/plans/arquitectura/17-graphify-opcional-no-critico.md` · **Rama**: `feat/graphify-opcional-y-refs-portables`
**Convención**: cada tarea es cerrada, verificable (done_cmd), aislada (files_touched disjuntos por oleada) y acotada.

Origen: directiva del autor (2026-10-22): «integra a SEELE el uso de Graphify de manera que sea
usable por este pero no crítico» y «borra de SEELE las referencias a rutas asumidas; pon en su
lugar ejemplos de repositorios similares open source». La oleada 0 apareció al intentar
compilar: `main` no construía.

## Oleada 0 — Build roto (bloqueante)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-00 | Deriva de versiones: `[workspace.dependencies]` pineaba los crates internos en `version = "0.2.0"` con `[workspace.package] version = "0.4.0"` y `Cargo.lock` en 0.2.0. Cargo no resuelve `seele-core = "^0.2.0"` contra 0.4.0: `cargo build` falla en local y el CI de `main` está rojo desde 2026-07-27 por la misma línea. Bump a `0.4.0` + `cargo update --workspace`. | `Cargo.toml`, `Cargo.lock` | `cargo build --workspace --locked` |

## Oleada 1 — Graphify opcional (ADR-17)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-01 | `seele graph export`: node-link JSON con observaciones, links, relaciones (`judged`→EXTRACTED, `pending`→INFERRED, resto fuera), hubs por proyecto/tipo, `--out`, `--project`, `--no-hubs`, `--include-deleted`, `--content-chars`. Builder puro con tests unitarios (ids, slug, fechas ISO sin `chrono`, filtrado de aristas colgantes). | `crates/seele-cli/src/commands/graph.rs` (nuevo), `commands/mod.rs`, `app.rs`, `main.rs` | `cargo test -p seele-cli graph` |
| T-02 | `--run-graphify` / `--graphify-bin` / `$GRAPHIFY_BIN` / PATH+PATHEXT, primera fuente configurada decide; ausencia → warning `graphify-not-found` y exit 0; fallo → exit 1; `--no-label` siempre; salida de Graphify a stderr. E2E con un graphify falso que sale 0 y otro que sale 3. | `crates/seele-cli/src/commands/graph.rs`, `crates/seele-cli/tests/graph_e2e.rs` (nuevo) | `cargo test -p seele-cli graph_e2e` |
| T-03 | Docs: `docs/GRAPHIFY.md` (formato, tabla de estados, instalación), README (bullet + docs), `docs/INDEX.md`, `CLAUDE.md` («Lo que ya corre»), `CHANGELOG.md` `[Unreleased]`. | `docs/GRAPHIFY.md` (nuevo), `README.md`, `docs/INDEX.md`, `CLAUDE.md`, `CHANGELOG.md` | `rg -n "graph export" README.md docs/INDEX.md CLAUDE.md CHANGELOG.md` |

## Oleada 2 — Referencias portables

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-04 | Investigación (workflow multi-agente, 2026-10-22): cómo citan repos hermanos, docs externas y herramientas opcionales los proyectos comparables (mem0, Letta, Graphiti, cognee, ENGRAM, basic-memory, servidor de memoria MCP, graphify; bat, delta, zoxide, jj, ripgrep, cargo/rustup; spec MCP, OpenTelemetry; CLAUDE.md/AGENTS.md). Cada ejemplo verificado contra la fuente. Resultado en `docs/CONVENTIONS.md` § «Cómo lo hacen otros». | `docs/CONVENTIONS.md` (nuevo) | revisión humana |
| T-05 | Sustituir toda ruta `C:/dev/...` en docs vivos por URL canónica (+ commit cuando se cita un archivo), enlace relativo o variable de entorno con default: `CLAUDE.md`, `docs/INDEX.md`, `docs/analysis/*.md`, `docs/design/INDEX.md`, `docs/plans/estrategia/v0.3-calidad-memoria/00-overview.md`, `docs/testing/guides/docker-localhost.md`, `docs/compendium/_sections/*.md` + compilados, `genesis/plans/**`. Devlogs y `docs/plans/executed/**` quedan como registro histórico con una nota al inicio de `docs/aegis/devlogs/README` o del INDEX. | los listados | `git grep -n -i "C:/dev/" -- ':!docs/aegis/devlogs' ':!docs/plans/executed' ':!docs/design/devlogs' ':!docs/CONVENTIONS.md'` vacío |
| T-06 | Código: `docs/compendium/_serve.mjs` resuelve su raíz con `import.meta.url`; `scripts/encode-mnema-lumen-counsel.py` toma el ledger de `$MNEMA_COST_LEDGER` (o `--ledger`) sin default absoluto. | `docs/compendium/_serve.mjs`, `scripts/encode-mnema-lumen-counsel.py` | `node docs/compendium/_serve.mjs --check` o arranque manual; `python scripts/encode-mnema-lumen-counsel.py --help` |

## Oleada 3 — Gates y cierre

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-07 | Gates del repo: tests, clippy `-D warnings`, fmt, residuo STELE. | — | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check && pwsh scripts/check-no-stele-residual.ps1` |
| T-08 | Instalar el binario (`cargo install --path crates/seele-cli --locked --force`), `seele doctor`, probar `graph export --run-graphify` contra una base real y verificar `GRAPH_REPORT.md`. | — | `seele graph export --project <p> --out <dir> --run-graphify --json` con `ok: true` |
| T-09 | Cierre AEGIS: devlog, CHANGELOG, `docs/INDEX.md`, mover este plan a `docs/plans/executed/tactica/`. Gate 2 humano. | `docs/aegis/devlogs/2026-10-22-graphify-opcional-y-refs-portables.md` | revisión humana |

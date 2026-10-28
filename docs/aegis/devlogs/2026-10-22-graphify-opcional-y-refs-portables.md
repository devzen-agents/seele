# Devlog 2026-10-22 — Graphify opcional (`seele graph export`) y referencias portables; build de `main` reparado

**Fecha**: 2026-10-22
**Agentes involucrados**: orquestador en consola (Claude Code) → ejecución directa de las tareas + workflow de investigación multi-agente (inventario, 6 ángulos de rastreo, verificación adversarial por ejemplo, síntesis, crítico) solo para T-04
**Plan origen**: directiva del autor en sesión (no hubo plan estratégico: alcance de patch)
**ADRs aplicados**: `docs/plans/arquitectura/17-graphify-opcional-no-critico.md` (ADR-17, propuesto en este ciclo)
**Tareas atómicas**: `docs/plans/tactica/2026-10-22-graphify-opcional-y-refs-portables.md` (T-00..T-09)
**Repos afectados**: `seele` (rama `feat/graphify-opcional-y-refs-portables`, base `a5127d5`)
**Migraciones DB**: ninguna
**Tests**: `seele-cli`: 7/7 unitarios nuevos + 5/5 e2e nuevos en verde (`cargo test --release -p seele-cli --bin seele graph` / `--test graph_e2e`); `cargo clippy --release --workspace --all-targets -- -D warnings` en verde; `cargo fmt --all -- --check` en verde; `scripts/check-no-stele-residual.ps1` OK. Resto del workspace: `cargo test --release --workspace --all-features` en verde en todas las suites tras refrescar los 10 snapshots de `seele-tui` (`SEELE v0.2.0` → `v0.4.0` en la barra de título: la misma deriva de versión que rompía el build había dejado las baselines desactualizadas).
**Build**: `main` NO compilaba (ver T-00); la rama compila e instala (`cargo install --path crates/seele-cli --locked`, binario slim).
**Gate 1**: tomado de la directiva del autor en sesión (alcance: «usable pero no crítico»; «sin rutas asumidas»). **Gate 2**: pendiente — este devlog y el plan quedan en `docs/plans/tactica/` hasta la aprobación humana.

## Resumen

Dos pedidos del autor y un bloqueo encontrado al empezar. (1) SEELE puede entregar su grafo a
[Graphify](https://github.com/Graphify-Labs/graphify) sin depender de él: `seele graph export`
escribe `graphify-out/graph.json` en el formato node-link que Graphify lee y, si se pide y hay una
CLI de Graphify a mano, la invoca para clusterizar y reportar; si no la hay, avisa y termina bien.
(2) Los documentos vivos del repo dejan de asumir rutas de la máquina del autor
(`C:/dev/protocols/AEGIS/...`, `C:/dev/tools/GRAIL/...`, etc.): 45 referencias en docs y scripts
pasan a URL canónica con commit, enlace relativo o variable de entorno, y `docs/CONVENTIONS.md`
fija la regla con ejemplos verificados de proyectos comparables. (0) `main` no construía desde
el commit de v0.4.0 por una deriva de versiones internas; corregida, y con ella el CI.

## Cambios entregados

### Build

- `Cargo.toml` — `[workspace.dependencies]`: los 13 pins `seele-* = { path, version = "0.2.0" }` → `0.4.0` (la versión real de `[workspace.package]`).
- `Cargo.lock` — `cargo update --workspace` (solo los crates internos pasan de 0.2.0 a 0.4.0; ninguna dependencia externa cambia).

### CLI (`crates/seele-cli`)

- `src/commands/graph.rs` — nuevo: `GraphCmd::Export`, builder puro `build_graph` (observaciones → nodos; `links` → aristas EXTRACTED; `memory_relations` `judged`→EXTRACTED, `pending`→INFERRED, `orphaned`/`ignored` fuera; hubs `seele_project_*` / `seele_type_*` con `--no-hubs`), `iso8601_from_millis` a mano (sin `chrono` en el CLI), `resolve_graphify_bin` (flag > `$GRAPHIFY_BIN` > PATH con `PATHEXT`; la primera fuente configurada decide), `run_graphify` (`cluster-only <out> --graph … --no-label [--no-viz]`, salida de Graphify reenviada a stderr), warning en banda `graphify-not-found`.
- `src/commands/mod.rs`, `src/app.rs`, `src/main.rs` — registro del subcomando `graph`.
- `tests/graph_e2e.rs` — nuevo: export con hubs y links, `--no-hubs --content-chars 0`, graphify ausente = warning + exit 0, graphify falso que sale 0 = `status: ran`, graphify falso que sale 3 = exit 1 con `graph.json` intacto.
- `tests/binary_e2e.rs` — solo reformateo de `cargo fmt` (una tupla partida en líneas); sin cambio semántico.
- `crates/seele-tui/tests/snapshots/views_snapshot__*.snap` (10) — baselines refrescadas: la única diferencia es `SEELE v0.4.0` en la barra de título.

### Storage

- `crates/seele-storage/src/observations.rs` — `get_embedding`: `#[allow(clippy::chunks_exact_to_as_chunks)]` con motivo (clippy 1.98 sugiere `as_chunks`, estable desde Rust 1.88; MSRV 1.85). Sin cambio de comportamiento.

### Docs

- `docs/GRAPHIFY.md` — nuevo: formato, tabla de estados/salidas, resolución del ejecutable, instalación.
- `docs/CONVENTIONS.md` — nuevo: reglas de referencia + tabla «qué se cambió» + «cómo lo hacen otros» (ejemplos verificados por la investigación) + comando de verificación.
- `docs/plans/arquitectura/17-graphify-opcional-no-critico.md` — nuevo (ADR-17).
- `docs/plans/tactica/2026-10-22-graphify-opcional-y-refs-portables.md` — nuevo (T-00..T-09).
- `README.md` (bullet + lista de docs), `CHANGELOG.md` (`[Unreleased]`: Added + 2 Fixed), `CLAUDE.md` («Lo que ya corre», referencias por URL, enlace a CONVENTIONS), `docs/INDEX.md` (guías nuevas, ADR-17, nota sobre registros históricos, enlace de cost-ledger por URL).
- Rutas de máquina eliminadas de: `docs/analysis/2026-06-09-*.md` (permalinks con commit a GRAIL `b7bb49e` y SEELE `91d275c`), `docs/design/INDEX.md`, `docs/plans/estrategia/v0.3-calidad-memoria/00-overview.md`, `docs/testing/guides/docker-localhost.md`, `docs/compendium/{SEELE-COMPENDIUM.md,SEELE-COMPENDIUM.html,_sections/00-frontmatter.md,_sections/18-build-ci.md,_sections/22-design-rationale.md}`, `genesis/plans/arquitectura/13-engram-compatibility.md`, `genesis/plans/estrategia/{00-INDEX.md,01-overview.md}`.
- Historial intacto a propósito: `docs/aegis/devlogs/*`, `docs/design/devlogs/*`, `docs/plans/executed/**`, `cost-ledger.jsonl` (nota en `docs/INDEX.md`).

### Scripts

- `docs/compendium/_serve.mjs` — `ROOT` resuelto con `dirname(fileURLToPath(import.meta.url))`.
- `scripts/encode-mnema-lumen-counsel.py` — ledger desde `$MNEMA_COST_LEDGER` (error claro si falta); el texto del counsel cita `templates/aesthetic-pillars.template.md` del repo LUMEN en vez de una ruta.

## Decisiones técnicas tomadas

1. **Archivo como único acoplamiento con Graphify** (ADR-17 D1): SEELE escribe el formato de salida de `graphify extract`; no importa Graphify ni conoce su versión. Verificado contra Graphify 0.9.67: `cluster-only`, `explain`, `path` y `graphify-mcp` leen el archivo sin conversión.
2. **Ausencia = warning y exit 0; fallo explícito = exit 1** (ADR-17 D2). Mismo contrato que `fake-embedder-fallback`. `--no-label` siempre: Graphify solo llama a un LLM para nombrar comunidades y el núcleo de SEELE no paga uno.
3. **Precedencia sin caída** para el ejecutable: `--graphify-bin` > `$GRAPHIFY_BIN` > PATH; una fuente configurada que no existe se reporta en vez de saltar a la siguiente (evita ejecutar un binario distinto del pedido).
4. **Hubs por proyecto y tipo por defecto** (ADR-17 D3): sin ellos una memoria real clusteriza en singletons. Medido: con la memoria de la Comisión CCHIA (45 observaciones, 0 links explícitos) el export da 52 nodos / 90 aristas y Graphify encuentra 6 comunidades.
5. **Fecha ISO a mano** en `seele-cli` en vez de sumar `chrono` al crate: 25 líneas con test de instantes conocidos (incluye año bisiesto y epoch negativo).
6. **Registros históricos no se reescriben**: devlogs y `executed/` conservan sus rutas; `docs/INDEX.md` lo advierte. Lo vigente (CLAUDE.md, README, docs, planes abiertos, génesis, scripts) cumple `CONVENTIONS.md`.
7. **`allow` puntual y no `as_chunks`** en `get_embedding`: respetar la MSRV declarada (1.85) pesa más que el lint nuevo.

## Incidentes durante la ejecución

- **`main` no compila** (`failed to select a version for the requirement seele-core = "^0.2.0"`): pins internos en 0.2.0 vs crates en 0.4.0. Confirmado en el CI de GitHub: rojo en `main` desde 2026-07-27 (corridas de 34–39 s, misma línea). Corregido (T-00).
- **Clippy 1.98** introduce `chunks_exact_to_as_chunks` y rompe `-D warnings` en código previo. Corregido con `allow` + motivo.
- **Artefacto bloqueado en Windows**: tras correr los e2e, `D:\cargo-target\seele\release\seele.exe` quedó con «Acceso denegado» (no se puede sobrescribir, borrar ni leer su ACL; ningún proceso lo tiene abierto; Defender sin detecciones; hay componentes de McAfee activos). `cargo install` falla en el paso final de copia. Workaround: el binario fresco `release\deps\seele.exe` (mismo build) se copió a `~/.cargo/bin/seele.exe` y se validó (`seele graph export --help`, export real con `--run-graphify`). Las pruebas del resto del workspace se corrieron con `--exclude seele-cli` para no chocar con ese archivo; las de `seele-cli` relevantes al cambio ya habían pasado antes del bloqueo.
- **El bloqueo resultó ser de la unidad D:** (sondeo con un crate trivial: en D: un `.exe` ya ejecutado no se puede sobrescribir, borrar ni leer su ACL; en C:, incluido `C:\Dev`, sí). El build y las pruebas completas del workspace se movieron al `target/` por defecto del repo en C: (1,9 GB). El directorio `D:\cargo-target` queda como residuo a borrar cuando el filtro lo suelte.
- **T-04 (investigación)**: el workflow devolvió 43 ejemplos verificados y 2 refutados; su crítico señaló tres cosas que se aplicaron: el enlace a `docs/COMPENDIUM.md` de GRAIL que este ciclo había puesto **no existe en el repo público** (comprobado con la API de GitHub en `b7bb49e` y en HEAD) y se reemplazó por la descripción del artefacto local; dos rutas con barra invertida en `genesis/` que el grep con `/` no veía; y la regla de verificación ahora cubre ambas barras. Abiertas para el autor (en `docs/CONVENTIONS.md` y la ADR): vendorizar el compendio y el informe de Cowork bajo `docs/analysis/sources/`, tags en aegis-protocol/lumen-protocol para citar por versión, y si un fallo de `spawn` de graphify debe ser warning (como jj/bat) en vez de error.
- **Avisos no tratados (fuera de alcance)**: `Cargo.lock` trae `chacha20 0.10.0` y `der 0.8.0` *yanked* en crates.io (cargo lo avisa con `--locked`); rustc 1.98 reporta el mensaje estándar del linker MSVC («Creando biblioteca … .lib/.exp») bajo el lint `linker_messages`.

## Uso y costo

Medición no disponible: el runtime de esta sesión (Claude Code, modelo `claude-fable-5-1`) no expone tokens por tarea de forma verificable; conciliación de coste pendiente. Sin fila en `cost-ledger.jsonl` (convención: no se inventan números).

| Modelo | Rol | Sesiones |
|---|---|---|
| claude-fable-5-1 (nodo central) | orquestador y ejecución directa de T-00..T-03, T-05..T-08 | 1 larga |
| subagentes del workflow (modelo de la sesión) | T-04: inventario (1) + rastreo (6 ángulos) + verificación (≤5 por ángulo) + síntesis + crítico | 1 workflow |

## Próximo

- **Gate 2 humano**: revisar el diff, decidir los commits (sugeridos: ① build: pins 0.4.0 + lock; ② feat: `seele graph export` + tests + docs + ADR-17; ③ docs: referencias portables + CONVENTIONS; ④ chore: allow clippy 1.98) y mover el plan a `docs/plans/executed/tactica/`.
- CI: con los pins corregidos el job vuelve a resolver; conviene fijar en `ci.yml` un toolchain explícito o aceptar que `-D warnings` siga el clippy de turno.
- Bumps pendientes de `chacha20`/`der` (yanked).
- Si el hand-off a Graphify se usa de verdad: considerar `seele graph export --watch` o integrar `graphify merge-graphs` con el grafo de código del repo consumidor (no antes de medir).

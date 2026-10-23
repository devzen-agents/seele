# ADR-17 — Graphify como consumidor opcional del grafo de memoria (usable, nunca crítico)

**Estado**: Propuesto · 2026-10-22 · **Táctica**: `docs/plans/tactica/2026-10-22-graphify-opcional-y-refs-portables.md` · **Doc de usuario**: `docs/GRAPHIFY.md`

## Contexto

SEELE ya tiene un grafo: observaciones como nodos, `links` tipados y `memory_relations` con
ciclo de juicio como aristas (ADR-16 D1). Lo que no tiene es clusterización, reporte ni
visualización, y el plan de mejoras GRAIL→SEELE las dejó en el Horizonte 3 (E12 GraphSnapshot,
E13 `seele-viz`, E18 `graph doctor`). [Graphify](https://github.com/Graphify-Labs/graphify)
(MIT, Python) hace exactamente eso sobre un `graph.json` en formato node-link de NetworkX:
comunidades Leiden, `GRAPH_REPORT.md`, `graph.html`, consulta por MCP y unión de grafos
(`merge-graphs`), con el código y la clusterización en local y el LLM solo para nombrar
comunidades. Reimplementar eso en Rust hoy no paga; depender de ello tampoco: SEELE es un
binario único, sin LLM en el núcleo y sin Python en el camino de guardado/búsqueda.

## Decisión 1 — SEELE escribe el formato de Graphify; no habla con Graphify

**Decisión**: `seele graph export [--project P] [--out DIR]` escribe
`DIR/graphify-out/graph.json` (node-link, `directed: true`) con observaciones, links, relaciones
juzgadas (`EXTRACTED`) y pendientes (`INFERRED`), más nodos hub por proyecto y por tipo
(`--no-hubs` los quita). El formato es el de salida de `graphify extract`, por eso
`cluster-only`, `graphify-mcp` y `merge-graphs` lo consumen sin conversión. El archivo se
escribe siempre, exista o no Graphify.

**Por qué**: un archivo es el acoplamiento más débil posible. SEELE no importa Graphify, no lo
instala, no conoce su versión; Graphify no necesita saber que el grafo vino de SEELE. Si
Graphify cambia o desaparece, el export sigue siendo un `graph.json` legible por NetworkX.

## Decisión 2 — La invocación es opt-in, la ausencia es un aviso y el fallo es un error

**Decisión**: `--run-graphify` localiza el ejecutable en este orden, y **la primera fuente
configurada decide**: `--graphify-bin` > `$GRAPHIFY_BIN` > `PATH` (con `PATHEXT` en Windows).
Una ruta explícita que no existe se reporta, no se salta a la siguiente fuente. Si no hay
ejecutable: aviso en stderr, warning `graphify-not-found` en el envelope JSON y **código 0**,
porque el export ya está escrito. Si el ejecutable existe y falla: **código 1** con el error de
Graphify, porque la persona lo pidió explícitamente. Siempre se pasa `--no-label`: nombrar
comunidades es el único paso de Graphify que llama a un LLM y el núcleo de SEELE no paga uno.

**Por qué**: es el mismo contrato de degradación que el embedder ONNX → `FakeEmbedder` con
`fake-embedder-fallback`: lo opcional que falta degrada en banda, nunca rompe. La precedencia
flag > variable de entorno > PATH es la convención de las CLIs que integran herramientas
externas (ver `docs/CONVENTIONS.md`); no caer de una fuente configurada a otra evita ejecutar
un binario distinto del que se pidió.

## Decisión 3 — Los hubs son parte del export, no un post-proceso

**Decisión**: por defecto se añaden nodos `seele_project_<slug>` y `seele_type_<slug>` con
aristas `in_project` / `has_type` de peso 0.5.

**Por qué**: una memoria real es escasa en links explícitos; sin hubs la detección de
comunidades devuelve un singleton por observación y el reporte no dice nada. Los hubs dan
estructura mínima con información que ya está en cada fila (proyecto y tipo); quien los quiera
fuera los quita con `--no-hubs` o los excluye en Graphify con `--exclude-hubs`.

## Consecuencias

- Nuevo subcomando en `seele-cli` (`commands/graph.rs`), sin crates ni dependencias externas
  nuevas; la fecha ISO se formatea a mano para no sumar `chrono` al CLI.
- E12/E13 del plan GRAIL→SEELE dejan de ser urgentes: Graphify cubre reporte y visualización
  mientras no haya una necesidad que exija hacerlo nativo.
- El `graph.json` contiene el contenido de las observaciones (ya sin `<private>`): vive donde
  vive la base, no se versiona.
- Si algún día se quiere consumir el reporte de Graphify desde SEELE (por ejemplo, guardar las
  comunidades como metadata), se abre otra ADR: esta solo exporta.

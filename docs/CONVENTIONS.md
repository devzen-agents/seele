# Convenciones de referencia — repos hermanos, docs externas y herramientas opcionales

**Estado**: vigente desde 2026-10-22 · **Origen**: auditoría de rutas hardcodeadas (50 ocurrencias
de `C:/dev/...` en 21 archivos, de las que 45 estaban en documentos vivos o scripts) y la investigación
multi-agente sobre cómo resuelven lo mismo proyectos open source comparables (§ Cómo lo hacen otros).

SEELE convive con otros repos del mismo autor (AEGIS, LUMEN, MNEMA, Cloven, KAIROS), con otro motor
de memoria (GRAIL) y con herramientas opcionales (Graphify). Ninguno de ellos vive en una ruta que
este repositorio pueda dar por sentada: un clon puede estar en cualquier disco, en cualquier sistema
operativo, o no existir. Estas reglas dicen cómo se los nombra.

## Reglas

1. **Nunca una ruta absoluta de una máquina.** Ni en docs, ni en scripts, ni en configuración
   versionada. Si una regla de `CLAUDE.md` o de un ADR necesita apuntar a algo externo, usa una de
   las formas de abajo. `git grep -n -i "C:/dev/"` sobre los documentos vivos debe devolver vacío
   (ver § Verificación).
2. **Repo hermano → URL canónica.** `https://github.com/<owner>/<repo>`. Si se cita un archivo
   concreto, permalink con commit o tag (`.../blob/<sha>/<ruta>`), porque la rama se mueve y la cita
   deja de ser verificable: el análisis GRAIL→SEELE fija GRAIL en el commit `b7bb49e`. **Nunca se
   inventa un enlace**: un archivo que no existe en el repo público (el compendio de GRAIL fue
   generado localmente por el autor) se nombra como artefacto local, no se enlaza (regla 7).
3. **Dentro del repo → enlace relativo** al archivo que lo cita (`[ADR-14](plans/arquitectura/14-evaluation-harness.md)`),
   nunca desde la raíz del disco. Es también la forma de `@import` de Claude Code: relativa al
   archivo que importa.
4. **Herramienta externa opcional → variable de entorno con default documentado + búsqueda en
   `PATH` + degradación en banda.** Orden fijo y escrito: flag explícito > variable de entorno >
   `PATH`; la primera fuente configurada decide (una ruta explícita que no existe se reporta, no se
   salta a la siguiente). La ausencia produce un código de warning en el envelope JSON
   (`graphify-not-found`, `fake-embedder-fallback`) y salida 0; nunca rompe el comando. Variables
   de SEELE que siguen esta regla: `GRAPHIFY_BIN` (`seele graph export --run-graphify`),
   `SEELE_EMBEDDER_DIR`, `SEELE_FAKE_EMBEDDER`, `SEELE_TOPIC_FAMILIES`, `SEELE_DB` (vía `--db`);
   del lado de los bridges de AEGIS/LUMEN: `SEELE_URL`, `SEELE_TOKEN`, `SEELE_TOKEN_FILE`.
5. **Secretos y ledgers externos → variable de entorno sin default**, con error claro si falta
   (`scripts/encode-mnema-lumen-counsel.py` exige `MNEMA_COST_LEDGER`; `seele serve
   --auth-bearer $VAR` lee el token del entorno para que no viaje en argv).
6. **Datos y caches → directorios estándar por plataforma** (crate `dirs`): `~/.seele/seele.db`,
   `%LOCALAPPDATA%\seele\embedder\` / `~/.cache/seele/embedder/`, siempre con override por
   variable o flag.
7. **Repos privados o no publicados (MNEMA, Cloven, KAIROS) se nombran, no se enlazan.** Se dice
   qué son y que no hacen falta para usar ni desarrollar SEELE. No se inventa una URL.
8. **Scripts resuelven su propia ubicación**, no la del autor: `import.meta.url` en Node
   (`docs/compendium/_serve.mjs`), `__file__` en Python, `$PSScriptRoot` en PowerShell.
9. **Los registros históricos no se reescriben.** Devlogs, cost-ledger y planes bajo `executed/`
   conservan las rutas con las que se escribieron; `docs/INDEX.md` lo advierte en una nota. Todo lo
   que un lector pueda tomar como vigente (`CLAUDE.md`, README, `docs/`, planes abiertos,
   `genesis/plans/` no ejecutados, scripts) sigue estas reglas.

## Qué se cambió (2026-10-22)

| Antes | Ahora |
|---|---|
| `C:/dev/protocols/AEGIS/AEGIS-PROTOCOL.md` | `https://github.com/orlando-vazquez-career/aegis-protocol/blob/main/AEGIS-PROTOCOL.md` |
| `C:/dev/protocols/AEGIS/guides/cost-ledger.md` | `.../aegis-protocol/blob/main/guides/cost-ledger.md` |
| `C:/dev/protocols/LUMEN/LUMEN-PROTOCOL.md` | `https://github.com/orlando-vazquez-career/lumen-protocol/blob/main/LUMEN-PROTOCOL.md` |
| `C:/dev/tools/GRAIL/docs/COMPENDIUM.md` | «compendio generado localmente por el autor sobre el clon de GRAIL en `b7bb49e`» + enlace al commit público; el archivo no existe en el repo de GRAIL, así que no se enlaza (pendiente de vendorizar en `docs/analysis/sources/`) |
| `C:/dev/tools/SEELE` (el propio repo) | «la raíz del repositorio» / `https://github.com/orlando-vazquez-career/seele` |
| `C:/dev/protocols/MNEMA/...`, `C:/dev/buddys/cloven/` | nombrados como repos privados del autor, no requeridos |
| `C:/dev/sandbox/experiments/<reporte>.md` | nombre del informe + «local del autor, fuera del repo» |
| `const ROOT = normalize('C:/dev/tools/SEELE/docs/compendium')` | `dirname(fileURLToPath(import.meta.url))` |
| `LEDGER = "C:/dev/protocols/MNEMA/cost-ledger.jsonl"` | `os.environ["MNEMA_COST_LEDGER"]`, error si falta |

## Cómo lo hacen otros

Ejemplos verificados contra la fuente el 2026-10-22 (workflow de 58 agentes: inventario, seis
ángulos de búsqueda, un verificador adversarial por ejemplo que abrió el archivo y comparó la cita;
43 ejemplos confirmados, 2 refutados y descartados). Los de GitHub llevan permalink al commit; los
de documentación oficial, a la página.

**Repos hermanos por URL; dentro del repo, enlaces relativos**

- letta-ai/letta `AGENTS.md`: el runtime vive en otro repo y se cita por URL (`letta-ai/letta-code`);
  el clon local se menciona como convención, no como ruta obligatoria.
  https://github.com/letta-ai/letta/blob/8816d3a8516893bfdc0fa14bfb1235226012e295/AGENTS.md
- coder/coder `AGENTS.md`: las guías se enlazan en relativo a la raíz del repo
  (`[FRONTEND_PATTERNS.md](.claude/docs/FRONTEND_PATTERNS.md)`), cada una con el ámbito al que aplica.
  https://github.com/coder/coder/blob/d8a4d30449be4b6934ef90892403870a74c4ed19/AGENTS.md
- Claude Code, *Memory › Import additional files*: «Relative paths resolve relative to the file
  containing the import»; un import fuera del directorio de trabajo es «external» y pide aprobación.
  https://code.claude.com/docs/en/memory#import-additional-files
- rust-lang/rust cita las RFC por URL con referencia al pie (`[rfc-401]`) dentro del propio código.
  https://raw.githubusercontent.com/rust-lang/rust/master/compiler/rustc_lint_defs/src/builtin.rs
- Permalinks: GitHub Docs (*Getting permanent links to files*: la tecla `y` fija la URL al commit),
  rust-lang/libc (exige permalinks en los mensajes de commit) y Linux (SHA + resumen de una línea al
  citar un commit). https://docs.github.com/en/repositories/working-with-files/using-files/getting-permanent-links-to-files ·
  https://github.com/rust-lang/libc/blob/main/CONTRIBUTING.md ·
  https://github.com/torvalds/linux/blob/master/Documentation/process/submitting-patches.rst

**Variables de entorno con default documentado**

- mem0: `MEM0_DIR` o `~/.mem0`.
  https://github.com/mem0ai/mem0/blob/2c81af3bbe27d9bccb09c772d583df9e9be72ee5/mem0/memory/setup.py#L10-L12
- ENGRAM: «The data directory defaults to ~/.engram; a nonblank ENGRAM_DATA_DIR overrides it and must
  be an absolute path» (una variable vacía cuenta como no definida).
  https://github.com/Gentleman-Programming/engram/blob/main/README.md
- Servidor de memoria oficial de MCP: `MEMORY_FILE_PATH`, default junto al servidor.
  https://github.com/modelcontextprotocol/servers/blob/main/src/memory/README.md
- codebase-memory-mcp: `CBM_CACHE_DIR`, default `~/.cache/codebase-memory-mcp`.
  https://github.com/DeusData/codebase-memory-mcp/blob/main/README.md
- rustup `RUSTUP_HOME` (`~/.rustup` o `%USERPROFILE%/.rustup`) y cargo `CARGO_HOME`: el patrón `*_HOME`.
  https://github.com/rust-lang/rustup/blob/5c8ff3f907c37688d20d9e7038a2f826042cd763/doc/user-guide/src/environment-variables.md#L8-L10 ·
  https://github.com/rust-lang/cargo/blob/master/doc/book/src/reference/environment-variables.md
- Codex: `CODEX_HOME`, default `~/.codex`. https://learn.chatgpt.com/docs/agent-configuration/agents-md.md
- graphiti (servidor MCP): `config.yaml` con expansión `${VAR}` y `${VAR:default}`.
  https://github.com/getzep/graphiti/blob/345e73161a61b0a4400f80c64e5bba6a584acb7b/mcp_server/README.md#L245-L247
- XDG Base Directory: `$XDG_CONFIG_HOME`, default `$HOME/.config`. https://specifications.freedesktop.org/basedir/latest/

**Herramientas opcionales: flag > variable > `PATH`, y degradación con aviso**

- bat: el pager se resuelve con `grep_cli::resolve_binary`; si no está, «Pager '…' not found,
  outputting to stdout instead» y sigue. `BAT_PAGER` pisa a `PAGER`.
  https://github.com/sharkdp/bat/blob/a9aa73d215455a479f072da75dd39f19826acfea/src/output.rs#L91-L126 ·
  https://github.com/sharkdp/bat/blob/a9aa73d215455a479f072da75dd39f19826acfea/README.md#L643-L650
- jj: herramientas externas por config (`merge-tools.*`, `:builtin` por defecto); un pager que
  «couldn't be found or couldn't be run» produce un warning con pista y el comando continúa.
  https://github.com/jj-vcs/jj/blob/531e8da8066271040b4b1e74ce17a8a941544cce/docs/config.md#L1167-L1194 ·
  https://github.com/jj-vcs/jj/blob/531e8da8066271040b4b1e74ce17a8a941544cce/cli/src/ui.rs#L396-L414
- zoxide: `which::which("fzf.exe")` en Windows y «could not find fzf, is it installed?» solo en el
  subcomando que lo necesita.
  https://github.com/ajeetdsouza/zoxide/blob/a1b102b5a5e43e854a81ab754548fec93a2c1679/src/util.rs#L26-L36
- cognee: `ENOLA_PATH`, luego el `Scripts/` del intérprete, luego `PATH`.
  https://github.com/topoteretes/cognee/blob/37f788bd287750e10d7610173e5332b3843f6ebd/cognee/tasks/code_graph/enola.py#L99-L114
- graphify se localiza a sí mismo con `shutil.which("graphify")` y cae al `Scripts/` o `bin/` del
  intérprete. https://github.com/Graphify-Labs/graphify/blob/8e09034743280ecc5ff2201b27c0ccae31f61966/graphify/install.py#L1502-L1537
- graphiti: un backend opcional es un extra; si falta, `ImportError` con la instrucción de
  instalación, y solo al usarlo.
  https://github.com/getzep/graphiti/blob/345e73161a61b0a4400f80c64e5bba6a584acb7b/graphiti_core/driver/falkordb_driver.py#L30-L35
- rust-analyzer busca en `PATH` con `env::split_paths`; which-rs expande `PATHEXT` en Windows.
  https://github.com/rust-lang/rust-analyzer/blob/0c4d74686b21f2b3bd8872b6d3cf36ad2e740d49/crates/toolchain/src/lib.rs#L130-L145 ·
  https://github.com/harryfei/which-rs/blob/8e84cb67761b6e19d2cfa512c5d55cbe5fd9e8bc/src/finder.rs#L277-L286
- `std::process` de Rust y el aviso CVE-2024-24576: los `.bat`/`.cmd` pasan por `cmd.exe` con un
  escapado especial; por eso `.exe` se prueba antes que `.cmd`.
  https://github.com/rust-lang/rust/blob/7189d39d9ce936881b8132c6addeaf3e295cd455/library/std/src/process.rs#L110-L130 ·
  https://blog.rust-lang.org/2024/04/09/cve-2024-24576.html

**Scripts que resuelven su propia ubicación**

- Node: `import.meta.url` + `fileURLToPath` (documentación de módulos ES); Vite lo usa para
  `VITE_PACKAGE_DIR` y Astro para leer su `package.json`.
  https://github.com/nodejs/node/blob/6d4646731e8d448dc5557e2911c27b8c8ca04174/doc/api/esm.md ·
  https://github.com/vitejs/vite/blob/a236886026c3a5de0680ea616de7de830d7a90e7/packages/vite/src/node/constants.ts ·
  https://github.com/withastro/astro/blob/d145d1961f0aa66c70a89a4bf759817b28517879/scripts/cmd/build.js

Descartados por el verificador: una cita de `modelcontextprotocol/go-sdk` y otra de
`opentelemetry-go` (el patrón estaba mal descrito). Los ejemplos tomados de clones locales se
sustituyeron por su permalink público.

## Verificación

```bash
# Documentos vivos sin rutas de máquina. Se excluyen a propósito los registros
# históricos y este mismo archivo, que cita el patrón prohibido en su tabla de ejemplos:
git grep -n -i "C:/dev/" -- ':!docs/aegis/devlogs' ':!docs/plans/executed' ':!docs/design/devlogs' ':!docs/CONVENTIONS.md'
# debe devolver vacío
```

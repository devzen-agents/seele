# Long-Markdown partial-edit regression

This is a **manual editor experiment with an automated byte-exact oracle**, not a
SEELE storage test. A reported Claude Code editing failure does not demonstrate a
SEELE defect. The harness does not invoke Claude, SEELE, MCP, or SQLite, and uses
only generated data. No model credentials or Python packages are required.

## Oracle tests (Linux, macOS, Windows)

Requires Python 3.9+. From the repository root:

```sh
python3 -B -m unittest discover -s scripts -p 'test_markdown_edit_regression.py' -v
```

On Windows use `py -3 -B` instead of `python3 -B` (or `python -B` when Python is on
PATH). These tests simulate outputs. They cover LF/CRLF and edits near the start,
middle, and end; a passing test **does not mean Claude Code was tested**.

## Real editor experiment

1. Choose a disposable parent directory **outside your memory directory and repo**.
   Use an absolute new child path, e.g. `/tmp/seele-markdown-lf-middle` on Unix or
   `C:\Users\you\AppData\Local\Temp\seele-markdown-lf-middle` on Windows.
   The parent must exist; the child must not. The harness refuses to overwrite
   an existing directory, including an earlier run.
2. From the repo root, generate one fixture (replace `<new-directory>`):

   ```sh
   python3 -B scripts/markdown_edit_regression.py prepare <new-directory> --eol lf --position middle
   ```

3. Open **only that disposable directory** in a separate Claude Code session.
   Do not attach SEELE MCP or expose real memory/database files. Keep normal
   permission protections enabled; do not bypass them for this test. Copy the
   exact edit prompt printed by `prepare`. Have Claude modify only
   `synthetic-long.md`. The fixture includes sentinels, repeated sections, tables,
   code fences, non-ASCII text, and more than 200 KiB of content.
4. After the editor finishes, run the verifier yourself from the repo root with
   the **same flags**. Do not ask the editor to generate or change the oracle:

   ```sh
   python3 -B scripts/markdown_edit_regression.py verify <new-directory> --eol lf --position middle
   ```

   Exit 0 means the entire file equals the original synthetic bytes plus exactly
   the requested substitution. Exit 1 means mismatch; exit 2 means an I/O/usage
   error. JSON reports byte counts, hashes, platform, and first differing offset.
   Checking only three sentinels would miss corruption between them, so the
   oracle compares every byte. It also rejects newline normalization, no-op,
   Unicode modification, a missing final newline, and append/truncation.
5. Repeat using a fresh directory for each combination of `--eol lf|crlf` and
   `--position start|middle|end` on each available OS. Record OS/version, Python
   version, `claude --version`, model, prompt, tool actually used by the editor,
   verifier JSON/exit code, and whether execution was manual or simulated.
   Keep generated files/results outside the repo. Do not treat LF/CRLF tests on
   Linux as Windows execution. Do not substitute an exact-replace script and
   report it as Claude behavior.

The verifier reconstructs the deterministic baseline from code and flags rather
than trusting an editable backup alongside the fixture. It does not prove which
editor made a change or that other files were untouched: enforce isolation and
review the session's file/tool activity separately. An unauthorized edit outside
this directory is not made safe by a passing byte comparison.

## Investigation and evidence (2026-10-28)

Baseline: `d32a661dd3548fe14f62d223ff09b56d1da0e3ec`.

- Inspected `crates/seele-setup/src/lib.rs`, agent installers, and CLI save/file
  write paths. Current implemented agent setup operates on JSON configuration;
  no production partial-Markdown editor was identified in those paths. Its
  existing atomic-write/concurrent-writer concerns are distinct from this test.
- No production fix is justified by the editor report alone. Rust code,
  database schemas, migrations, and real memories remain unchanged.
- Linux / Python 3.12.14: four oracle unit tests pass; six valid synthetic outputs
  accepted and 54 corrupted outputs rejected, plus CLI/error/overwrite checks.
- Claude Code was unavailable (`claude` not on PATH): **real-editor experiment
  not run**. Windows/macOS: **not run**. No cross-platform claim is made.
- No CI job is added: these local oracle checks do not exercise Claude. They can
  be run on each target OS without Rust, an LLM service, or SEELE configuration.
- No full Rust workspace test/clippy run: no Rust changes; `cargo` was not on the
  current PATH. Existing cached toolchains were not installed or reconfigured.

This is a local reviewable test addition, not a published release or closed
sprint. Real-editor/platform evidence and human review remain pending.

# Synthetic long-Markdown partial-edit regression

Authorized scope: implement the bulletin's synthetic regression, not a repair of
an unproven SEELE defect. User approved executing the proposed test (Gate 1).

1. Generate deterministic long UTF-8 Markdown with start/middle/end sentinels,
   Unicode, code fences, tables, and LF/CRLF variants in a new isolated directory.
2. Verify the complete result byte-for-byte against one authorized replacement;
   reject truncation, no-op, newline conversion, and unrelated changes.
3. Unit-test the oracle with simulated correct and corrupt editor outputs.
4. Document the separate manual Claude Code experiment and platform evidence.

Do not invoke SEELE, read memories, open SQLite, migrate, publish, or deploy.
No production code change unless investigation demonstrates a defect. Automated
oracle tests are not evidence of Claude Code behavior. Windows/macOS executions
must remain explicitly unverified until run there. Gate 2 remains user review;
this plan stays in place rather than being marked as an executed sprint.

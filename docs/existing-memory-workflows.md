# Existing-memory workflows

SEELE CLI, HTTP and MCP use the same SQLite storage. Running an HTTP server does
not make an ephemeral workspace durable.

For an existing memory database, use an explicit absolute path and opt in to the
startup preflight:

```sh
seele --db /absolute/path/seele.db --require-existing-db list
```

The guard refuses a missing path or directory, before creating a database. It is
available globally, including for `save`, `serve`, and `mcp`. Without the flag,
normal first-use database initialization remains unchanged. `backup` always
requires an existing source and continues to refuse an existing destination.
On Windows, pass the equivalent absolute native path (quote paths with spaces).

This is not a database identity, freshness, integrity or concurrency guarantee.
An existing but stale/wrong database still needs to be detected by comparing
record IDs and known checkpoint revisions. A concurrent path replacement after
the preflight is outside this guard's scope. Changing HOME changes the default
`~/.seele/seele.db`; do not rely on that default across isolated environments.

Keep a coherent SQLite backup outside ephemeral execution storage, with a
verified manifest and version. Compare both copies before reconciling; do not
silently replace a newer or divergent database. A successful process restart
is not proof of persistence after the execution environment is recreated.

# Reference

Use these pages when scripting, integrating an agent, or writing a client.

- [CLI reference](./cli.md) — every command, flag, and output format.
- [HTTP and WebSocket API](./http-api.md) — routes, payloads, and the change feed.
- [Design directory format](./yaml.md) — the YAML a design is stored as.
- [Shipped catalogue](./catalogue.md) — component types, behaviours, and signals.

The source of truth remains the typed Rust API and `optimist <command> --help`.
The repository enables `#![deny(missing_docs)]`, so every public Rust item is
documented and the generated rustdoc is warning-free.

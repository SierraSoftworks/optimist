# Optimist fuzz targets

This `cargo-fuzz` package covers the untrusted text and network payloads Optimist
accepts:

- `squiggle_programs` parses and lints author-supplied Squiggle source.
- `system_documents` decodes system and component YAML documents.
- `mutation_batches` decodes the JSON mutation arrays accepted by the API.

Squiggle and YAML inputs are capped at 16 KiB. Mutation JSON is also capped at
16 KiB, 16 nested collections, 32 items per collection, and 512 bytes per
string, so the fuzzer spends its budget on structure rather than input growth.

Run a bounded smoke pass from the repository root after installing nightly Rust
and `cargo-fuzz`:

```sh
cargo +nightly fuzz run squiggle_programs -- -max_len=16384 -runs=1000
cargo +nightly fuzz run system_documents -- -max_len=16384 -runs=1000
cargo +nightly fuzz run mutation_batches -- -max_len=16384 -runs=1000
```

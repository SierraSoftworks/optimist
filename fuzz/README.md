# Optimist fuzz targets

This cargo-fuzz package contains only targets backed by implemented Optimist behavior:

- `entity_edge_ids` parses canonical `EntityId` and `EdgeId` text.
- `tagged_aggregates` decodes and round-trips `Node`, `NodePayload`, `Edge`, `EdgePayload`, and `Observation` JSON.
- `command_replay` decodes bounded command arrays, executes them against two in-memory catalogs, and checks deterministic results and retries.
- `yaml_documents` parses bounded project, entity, and scenario YAML documents.
- `dependence_model` decodes bounded project dependence documents, round-trips them, and checks deterministic matrix/project validation.

The checked-in seed files and dictionaries use the `v1_` prefix. Keep old seeds when adding a new corpus version so previously discovered shapes remain covered.

JSON is rejected before serde decoding when it exceeds 16 KiB, 16 nested collections, 32 items in any collection, or 512 raw bytes in any string. Command scripts are additionally limited to 16 requests. ID inputs are limited to 128 bytes.

Run a bounded smoke pass from the repository root after installing nightly Rust and `cargo-fuzz`:

```sh
cargo +nightly fuzz run entity_edge_ids fuzz/corpus/entity_edge_ids -- -max_len=128 -runs=1000 -dict=fuzz/dictionaries/entity_edge_ids_v1.dict
cargo +nightly fuzz run tagged_aggregates fuzz/corpus/tagged_aggregates -- -max_len=16384 -runs=1000 -dict=fuzz/dictionaries/tagged_aggregates_v1.dict
cargo +nightly fuzz run command_replay fuzz/corpus/command_replay -- -max_len=16384 -runs=1000 -dict=fuzz/dictionaries/command_replay_v1.dict
cargo +nightly fuzz run probability_sampling fuzz/corpus/probability_sampling -- -max_len=16384 -runs=1000 -dict=fuzz/dictionaries/probability_sampling_v1.dict
cargo +nightly fuzz run dependence_model fuzz/corpus/dependence_model -- -max_len=16384 -runs=1000 -dict=fuzz/dictionaries/dependence_model_v1.dict
```

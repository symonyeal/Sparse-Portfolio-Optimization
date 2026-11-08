# Advisory Rust structure checks

Sentrux 0.5.7 scans the first-party Rust files selected by `scope.json`. Numerical modules may not
depend on orchestration modules. Its layer convention uses the larger order for the numerical layer.
One existing component joins `bd`, `data`, `bench` and `oos`, including test imports; the baseline
retains it. A green structural result does not establish numerical correctness.

Use the shared local runner (`sentrux/scan.py` in the machine's shared tools folder):

```text
python "<shared runner>" "<repository>" "<new run label>"
```

The runner copies tracked Rust source and Cargo manifests to a separate indexed scan directory,
then checks these rules and compares `baseline.json`. It excludes tests, input/result bundles,
archives, vendor code and build output; inline Rust tests remain. It reads current working-tree
bytes of tracked source, including edits. New source files must be staged before scanning.
Logs and per-file SHA-256 hashes stay beside the shared runner under `evidence/`.
Append `--baseline` only to approve a replacement baseline after reviewing changes.

The pinned Windows binary and grammar archive are installed locally, with analytics disabled.
Other machines need the same Sentrux version and shared runner. The CLI reads all indexed files
when invoked directly on the repository; that is a different scan scope.

Validation on 2026-11-06 rejected a planted reverse import in an isolated source copy. The Rust
resolver misses some qualified cross-crate imports in the other trial projects, so these checks
remain advisory. No CI gate is added; the existing solver, format, Clippy and test checks remain acceptance.

The shared runner also checks Cargo's declared workspace dependencies against layers. This repository
has one crate, so that additional crate-level check reports not applicable; its file-level analysis
still comes from Sentrux.

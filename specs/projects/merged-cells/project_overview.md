---
status: complete
---

# Merged Cells (Core / Model)

Implement merged-cell support in the IronCalc core engine (the `base` Rust
crate) — the "API to deal with merged cells" portion of
[issue #61](https://github.com/ironcalc/IronCalc/issues/61).

## Context

Issue #61 (Apr 2024) plans merged cells in two phases:

- **Core module:** import merged cells · API to deal with merged cells · export merged cells
- **Web app:** show merged cells · merge/unmerge UI

Import and export **already exist and are tested** (`xlsx/src/import/worksheets.rs`
`load_merge_cells`, `xlsx/src/export/worksheets.rs` `<mergeCells>` round-trip,
`xlsx/tests/test.rs::test_exporting_merged_cells`). The storage field
`Worksheet.merge_cells: Vec<String>` already exists. What is missing is the
**API and all model behavior** that a live merge list implies.

Today `merge_cells` is effectively a **dead field**: nothing reads it, validates
it, or keeps it consistent. It survives an xlsx round-trip, but any structural
edit (insert/delete/move rows or columns), cell edit, or range clear silently
corrupts it. This project makes merged cells a first-class, consistent part of
the model.

## Scope

- **In scope:** the core/model API to create, query, and remove merges;
  undo/redo; keeping merges consistent under every existing model operation;
  language bindings (wasm/python/nodejs) so a UI can drive it.
- **Out of scope:** any UI. The consumer is building their own UI and only needs
  the model. Rendering, selection behavior, merge/unmerge menu items, and
  keyboard navigation across merges are the UI's responsibility. The model
  exposes enough (a query API) for a UI to implement those.

## Why

Merged cells are a baseline expectation of any spreadsheet and a listed IronCalc
1.0 milestone. The storage and file I/O are already done, so the remaining core
work is self-contained and unblocks the feature for any front end (including the
first-party web app later).

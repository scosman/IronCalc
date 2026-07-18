---
status: complete
---

# Implementation Plan: Merged Cells (Core / Model)

Phases are ordered foundational-first. Each is a self-contained, reviewable unit
that builds and tests green on its own. See `architecture.md` for the design and
exact file:line targets.

## Phases

- [x] **Phase 1 — Core API + undo/redo.** `MergeCell` type (`types.rs`);
  `Worksheet` helpers `merge_cells_parsed` / `merge_at` + A1<->index conversion
  (`worksheet.rs`); base `Model::{merge_cells, unmerge_cells, get_merge_cells,
  get_merge_cell}` with full validation (bounds, degenerate, overlap,
  array/spill collision); `UserModel` wrappers; `Diff::MergeCells` /
  `Diff::UnmergeCells` (`history.rs`) handled in both undo and redo matches
  (`undo_redo.rs`); unit tests for API, validation, and undo/redo symmetry.

- [x] **Phase 2 — Edit & clear guards.** Reject writes to covered cells in
  `set_user_input` (`common.rs`); unmerge fully-contained regions in
  `range_clear_all` (bundled into its diff list) while leaving
  `range_clear_contents` / `range_clear_formatting` merge-preserving; tests for
  both guards including undo.

- [ ] **Phase 3 — Displacement.** `displace_merge_cells` (`actions.rs`) wired
  into all six sites (insert/delete/move × rows/columns) beside the existing CF
  displacement; grow/shift/shrink/drop rules; drop-on-collapse-to-1×1; move-split
  guard mirroring the array-formula guard. One test per displacement case, each
  with undo/redo; row/column symmetry.

- [ ] **Phase 4 — Bindings + round-trip.** Expose the four methods in the wasm,
  python, and nodejs bindings; `MergeCell[]` serialization for JS; verify the
  existing xlsx round-trip test and add an import→merge-via-API→export→re-import
  case; note the Phase-2 clipboard limitation in docs.

## Deferred (not in this project)

- Clipboard merge fidelity (copy/paste of regions).
- Any UI (rendering, selection, navigation, menus) — consumer-owned.

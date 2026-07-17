---
status: complete
---

# Functional Spec: Merged Cells (Core / Model)

This spec defines the behavior of merged cells at the model layer. It is written
against the current tree (branch identical to `main`) where `merge_cells` exists
as storage but has no behavior.

## Terminology

- **Merged region** — a rectangle of cells that display and behave as one.
  Stored as an A1 range string (e.g. `"B2:D4"`) in `Worksheet.merge_cells`.
- **Anchor** — the top-left cell of a region. It is the only cell that holds a
  value. Its address is the region's address.
- **Covered cell** — any non-anchor cell inside a region. Always empty of
  content; may retain its own style so it re-appears cleanly on unmerge.

The model follows Excel semantics throughout: "you cannot change part of a
merged cell," merging keeps only the top-left value, and a merged region is
addressed by its anchor.

## Features

### 1. Merge a rectangle

`merge_cells(sheet, row, column, width, height)` creates a region.

- Keeps only the **anchor** value; all covered cells have their **content
  cleared** (Excel discards it). Covered cells keep their **style**.
- Appends the normalized A1 range to `Worksheet.merge_cells`.
- Undoable/redoable as a single history entry that restores the discarded
  covered content on undo.

### 2. Unmerge a region

`unmerge_cells(sheet, row, column)` removes the region that contains
`(row, column)` (anchor or any covered cell).

- Removes the range from `Worksheet.merge_cells`. Covered cells are already
  empty; no content is synthesized.
- Undoable/redoable.

### 3. Query merges

- `get_merge_cells(sheet) -> Vec<MergeCell>` — all regions on a sheet, as
  normalized `{ row, column, width, height }`.
- `get_merge_cell(sheet, row, column) -> Option<MergeCell>` — the region
  covering a given cell (anchor **or** covered), or `None`. This is the primary
  hook a UI uses to redirect selection/editing to the anchor and to render a
  region as one box.

### 4. Consistency under every model operation

The merge list is kept correct across all existing operations (this is the bulk
of the work — see Edge Cases and the architecture doc).

## Public interface (Rust `UserModel`)

```rust
pub struct MergeCell {
    pub row: i32,
    pub column: i32,
    pub width: i32,   // >= 1, columns spanned
    pub height: i32,  // >= 1, rows spanned
}

impl UserModel {
    pub fn merge_cells(&mut self, sheet: u32, row: i32, column: i32, width: i32, height: i32) -> Result<(), String>;
    pub fn unmerge_cells(&mut self, sheet: u32, row: i32, column: i32) -> Result<(), String>;
    pub fn get_merge_cells(&self, sheet: u32) -> Result<Vec<MergeCell>, String>;
    pub fn get_merge_cell(&self, sheet: u32, row: i32, column: i32) -> Result<Option<MergeCell>, String>;
}
```

The same four methods are mirrored in the wasm, python, and nodejs bindings.
`get_merge_cells` serializes to `MergeCell[]` for JS consumers.

## Behavior contracts and edge cases

### Merge validation (all return `Err(String)`, no partial mutation)

1. **Out of bounds** — any part of the rect outside `[1, LAST_ROW]` ×
   `[1, LAST_COLUMN]`.
2. **Degenerate** — `width < 1 || height < 1`, or `width == 1 && height == 1`
   (a 1×1 merge is meaningless).
3. **Overlap** — the rect intersects an existing region at all. (Excel forbids
   overlapping merges; only fully-nested-identical is a no-op, which we also
   reject as an overlap to keep it simple.)
4. **Array/spill collision** — the rect intersects an array-formula anchor or a
   dynamic-array spill. Excel forbids merging over an array formula. Mirrors the
   existing array-split guards used by insert/delete.
5. **Invalid sheet** — propagated from `worksheet(sheet)?`.

On success, covered-cell content is discarded (recorded for undo).

### Unmerge

- If `(row, column)` is not inside any region → `Ok(())` no-op (idempotent), OR
  `Err` if strictness is preferred. **Decision: no-op** (friendlier for a UI that
  calls unmerge on the current selection unconditionally).

### Editing a covered cell — `set_user_input`

- Writing to a **covered** (non-anchor) cell returns
  `Err("cannot edit a cell inside a merged region; edit the anchor at <A1>")`.
- Writing to an **anchor** works normally.
- Rationale: the model has no UI selection concept; a covered-cell write is
  almost always a mistake. The UI is expected to redirect the caret to the
  anchor via `get_merge_cell` before calling `set_user_input`. (Alternative
  considered: silently redirect the write to the anchor. Rejected — it hides the
  merge from scripted callers and is surprising. Rejecting is explicit and the
  UI hook makes redirection trivial.)

### Clearing ranges

- `range_clear_all(area)` — Excel's "Clear All" unmerges. Any region **fully
  contained** in `area` is removed (unmerged) in addition to clearing content
  and formatting. A region only **partially** overlapped by `area` is left
  intact (its anchor content still clears if the anchor is in `area`).
- `range_clear_contents(area)` — clears anchor content; **does not** unmerge.
- `range_clear_formatting(area)` — unchanged; does not touch merges.

### Structural edits (insert / delete / move rows & columns)

Merges shift with the grid, mirroring how conditional-formatting ranges are
displaced today:

- **Insert rows/columns** before or through a region → shift and/or grow it.
- **Delete rows/columns:**
  - Deletion entirely before a region → shift it.
  - Deletion overlapping a region → shrink it.
  - Deletion that removes the whole region, **or shrinks it to 1×1 → drop the
    merge** (a 1×1 merge is degenerate).
- **Move rows/columns** (the `move_columns_action` / `move_rows_action` paths):
  a move that would **split** a region (move some of its rows/columns but not
  all) is **blocked** with an error, mirroring the array-formula guard. A move
  that shifts a whole region intact updates the range.

Insert/delete that would split a region are likewise blocked with a clear error
(same pattern as `"...would break an array formula"`), OR handled by grow/shrink
where unambiguous. **Decision:** insert/delete **never** split (they grow/shrink,
or drop on collapse); only **move** can be ambiguous and is blocked.

### Sheet-level operations (already correct — verify only)

- **Duplicate sheet** already deep-copies `merge_cells` (`new_empty.rs`).
- **Delete sheet** undo already restores `merge_cells` wholesale
  (`undo_redo.rs`). No new work; add a regression test.

### Copy / paste — deferred (Phase 2, documented limitation)

Phase 1 does **not** carry merge information through the clipboard. Copying a
region and pasting elsewhere pastes cell values/styles only; it does not
reproduce the merge, and pasting onto an existing merge does not unmerge it.
This is an explicit, documented limitation, isolated from the core API so it can
be added later without churn.

## Evaluation semantics (no engine change)

A merged region stores a value only in its anchor; covered cells are genuinely
empty. Formulas referencing a covered cell already resolve to empty/0 with no
special handling, because the merge operation clears covered content and import
never relocates values. **The formula evaluation engine is untouched.** Merging
is a presentation + edit-guard concern only.

## Constraints

- **Serialization compatibility:** `Worksheet.merge_cells` stays `Vec<String>`
  (A1 ranges). No bitcode format change, no change to the xlsx import/export
  paths.
- **Performance:** merge counts per sheet are small (typically < a few hundred).
  Linear scans over `merge_cells` for coverage lookups are acceptable; no index
  is required in Phase 1.
- **Totality of undo/redo:** every new operation is expressed as a `Diff` and
  handled in both the undo and redo matches (compiler-enforced exhaustiveness).

## Out of scope

- All UI (rendering, selection, menus, navigation across merges).
- Clipboard merge fidelity (Phase 2).
- Merge-aware auto-fill and merge-aware border rendering as a unified box (the
  UI renders borders; the model stores per-cell styles as today).

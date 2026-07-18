---
status: complete
---

# Phase 1: Core API + undo/redo

## Overview

Introduce the merged-cells core model API and its undo/redo support, mirroring
the conditional-formatting diff shape. This phase adds the typed `MergeCell`
value, borrow-only worksheet helpers that convert between the stored A1 range
strings and the typed view, the four base `Model` primitives with full
validation, thin `UserModel` wrappers that record history, and the two new
`Diff` variants handled exhaustively in both undo and redo.

Out of scope for this phase (later phases): the edit guard in `set_user_input`,
unmerge-on-clear in `range_clear_all`, structural displacement, and the wasm /
python / nodejs bindings.

## Steps

1. `base/src/types.rs` — add the value type near `SheetProperties`:
   ```rust
   #[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
   pub struct MergeCell {
       pub row: i32,
       pub column: i32,
       pub width: i32,  // >= 1
       pub height: i32, // >= 1
   }
   ```

2. `base/src/worksheet.rs` — add A1<->index free functions and borrow-only
   helpers:
   ```rust
   pub(crate) fn merge_cell_to_a1(m: &MergeCell) -> Option<String>;   // "B2:D4"
   pub(crate) fn a1_to_merge_cell(range: &str) -> Option<MergeCell>;  // parse, defensive

   impl Worksheet {
       pub(crate) fn merge_cells_parsed(&self) -> Vec<MergeCell>;     // skips unparseable
       pub(crate) fn merge_at(&self, row: i32, column: i32) -> Option<MergeCell>;
       pub(crate) fn merge_contains(m: &MergeCell, row: i32, column: i32) -> bool;
       // mutation helpers reused by undo/redo:
       pub(crate) fn apply_merge(&mut self, m: &MergeCell);           // clear covered + append range
       pub(crate) fn remove_merge_at(&mut self, row: i32, column: i32) -> Option<MergeCell>;
   }
   ```

3. `base/src/merge_cells.rs` (new module, wired in `lib.rs`) — the four base
   `Model` primitives:
   ```rust
   impl<'a> Model<'a> {
       pub fn merge_cells(&mut self, sheet: u32, row: i32, column: i32, width: i32, height: i32)
           -> Result<Vec<Vec<Option<Cell>>>, String>;   // full validation; returns old covered content
       pub fn unmerge_cells(&mut self, sheet: u32, row: i32, column: i32)
           -> Result<Option<MergeCell>, String>;         // no-op -> Ok(None)
       pub fn get_merge_cells(&self, sheet: u32) -> Result<Vec<MergeCell>, String>;
       pub fn get_merge_cell(&self, sheet: u32, row: i32, column: i32) -> Result<Option<MergeCell>, String>;
   }
   ```
   Validation order (each `Err(String)`, no mutation before all pass): sheet
   exists -> bounds (`row>=1 && column>=1 && last_row<=LAST_ROW &&
   last_col<=LAST_COLUMN`, computed in i64 to avoid overflow) -> degenerate
   (`width>=1 && height>=1 && !(1x1)`) -> overlap (rect-intersect over
   `merge_cells_parsed`) -> array/spill collision (any `Cell::ArrayFormula` /
   `Cell::SpillCell` in the rect). On success record `old_covered` (row-major
   over the rect, anchor slot = `None`) then `apply_merge`.

4. `base/src/user_model/merge_cells.rs` (new module, `mod merge_cells;` in
   `user_model/mod.rs`) — the four `UserModel` wrappers. `merge_cells` /
   `unmerge_cells` call the base method, push a `Diff`, and
   `evaluate_if_not_paused()`. `unmerge_cells` records the diff with the removed
   region's anchor coords so redo/undo are unambiguous; no diff when nothing was
   merged.

5. `base/src/user_model/history.rs` — add `Diff::MergeCells { sheet, row,
   column, width, height, old_covered: Vec<Vec<Option<Cell>>> }` and
   `Diff::UnmergeCells { sheet, row, column, width, height }`.

6. `base/src/user_model/undo_redo.rs` — handle both variants in both matches:
   - undo `MergeCells`: `remove_merge_at` + restore `old_covered`.
   - redo `MergeCells`: `apply_merge`.
   - undo `UnmergeCells`: `apply_merge` (re-merge; covered already empty).
   - redo `UnmergeCells`: `remove_merge_at`.
   All set `needs_evaluation = true`.

7. `base/src/lib.rs` — `mod merge_cells;` and re-export `MergeCell` if needed
   (already reachable via `pub mod types`).

## Tests

New `base/src/test/user_model/test_merge_cells.rs` (registered in
`test/user_model/mod.rs`):

- `merge_reflected_in_queries` — merge B2:D4; `get_merge_cells` returns one
  `MergeCell{2,2,3,3}`; `get_merge_cell` returns it for the anchor and every
  covered cell and `None` outside.
- `merge_clears_covered_keeps_anchor` — anchor value retained; covered cells
  become empty.
- `merge_undo_redo` — undo restores covered content and removes the region;
  redo re-applies (region back, covered cleared).
- `unmerge_removes_region` — unmerge via a covered cell; region gone; undo
  restores it; redo removes it again.
- `unmerge_noop_when_not_merged` — unmerge on an unmerged cell is `Ok(())` and
  records no history.
- `validation_out_of_bounds`, `validation_degenerate_1x1`,
  `validation_zero_or_negative_dims`, `validation_overlap`,
  `validation_array_spill` — each returns `Err` and leaves `get_merge_cells`
  unchanged (empty).
- `validation_invalid_sheet` — `Err` from a bad sheet index.

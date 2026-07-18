---
status: complete
---

# Phase 3: Displacement

## Overview

Keep the merge list correct across the six structural-edit sites (insert/delete/move
× rows/columns), mirroring how conditional-formatting ranges are displaced today.
Add a single `displace_merge_cells` engine in `base/src/actions.rs`, wire it in
next to the existing `displace_cf_ranges` call at all six sites, and add a
move-split guard mirroring the array-formula guard. Every transformation is
undoable/redoable.

Displacement rules (functional spec §"Structural edits"):

- Insert before a region → shift; insert through a region → grow.
- Delete before → shift; delete overlapping → shrink; delete removing the whole
  region **or** shrinking it to 1x1 → drop the merge.
- Insert/delete never split (they grow/shrink or drop on collapse).
- Move that shifts a whole region intact → update the range; move that would
  split a region (some but not all of its rows/columns) → blocked with an error,
  mirroring the array-formula split guard.

## Steps

1. `base/src/actions.rs` — new free helpers near `displace_cf_col`:
   - `move_line(line, moved, delta) -> i32` — single-line move transform (same
     rule the `RowMove`/`ColumnMove` arms of `displace_cf_row`/`_col` use).
   - `displace_delete_insert_span(start, len, p, delta) -> Option<(i32, i32)>` —
     one axis under insert (`delta > 0`: shift/grow) or delete (`delta < 0`:
     shift/shrink), `None` when the axis is entirely removed.
   - `finish_span(MergeCell) -> Option<MergeCell>` — drops a region collapsed to
     1x1 by a deletion.
   - `displace_merged_region(&MergeCell, &DisplaceData, sheet) -> Option<MergeCell>`
     — dispatches on the variant: Row/Column use the span helper + `finish_span`;
     RowMove/ColumnMove displace both edges via `move_line` (never drop; the guard
     guarantees the region is whole); other variants return the region unchanged.

2. `base/src/actions.rs` — `Model::displace_merge_cells(&mut self, sheet, &DisplaceData)`:
   reads `merge_cells_parsed()`, maps each region through `displace_merged_region`,
   drops `None`s, converts survivors back to A1 via `merge_cell_to_a1`, writes back
   the new `Vec<String>`. Shape mirrors `displace_cf_ranges` (returns `()`,
   sheet-missing is a no-op).

3. Wire `self.displace_merge_cells(sheet, &disp);` immediately after the existing
   `self.displace_cf_ranges(sheet, &disp);` at all six sites: `insert_columns`,
   `delete_columns`, `insert_rows`, `delete_rows`, `move_column_unchecked`,
   `move_row_unchecked`.

4. `base/src/actions.rs` — move-split guard:
   - `can_move_columns_merge` / `can_move_rows_merge`, mirroring
     `can_move_columns_action` / `can_move_rows_action` but iterating
     `merge_cells_parsed()` and testing the region's column/row span with the same
     `interval_is_safe` predicate.
   - In `move_columns_action` / `move_rows_action`, right after the array-formula
     guard, reject with `"Cannot move columns because that would split a merged cell"`
     / `"Cannot move rows because that would split a merged cell"`.

5. Undo capture for deletions (deletes shrink/drop merges, which re-running
   `insert_*` cannot reverse). Add `old_merge_cells: Vec<String>` to
   `Diff::DeleteRows` / `Diff::DeleteColumns` (`history.rs`); snapshot the sheet's
   `merge_cells` in `UserModel::delete_rows` / `delete_columns` (`common.rs`);
   restore it in the undo handlers (`undo_redo.rs`) after the rows/columns are
   re-inserted. Redo re-runs `delete_*` (correct forward displacement), so the
   field is ignored on the redo side. Insert and move round-trip by re-running the
   inverse op, so they need no snapshot.
   - Update the `Diff::DeleteRows` / `Diff::DeleteColumns` destructures in
     `test_batch_row_column_diff.rs` for the new field.

## Tests (`base/src/test/user_model/test_merge_cells.rs`, each with undo/redo)

- `insert_rows_before_region_shifts` / `insert_columns_before_region_shifts`
- `insert_rows_through_region_grows` / `insert_columns_through_region_grows`
- `delete_rows_before_region_shifts` / `delete_columns_before_region_shifts`
- `delete_rows_overlapping_region_shrinks` / `delete_columns_overlapping_region_shrinks`
- `delete_rows_covering_region_drops` / `delete_columns_covering_region_drops`
- `delete_rows_collapsing_to_1x1_drops` / `delete_columns_collapsing_to_1x1_drops`
- `move_rows_whole_region_intact` / `move_columns_whole_region_intact`
- `move_rows_splitting_region_blocked` / `move_columns_splitting_region_blocked`

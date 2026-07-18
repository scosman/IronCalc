---
status: complete
---

# Phase 2: Edit & clear guards

## Overview

Phase 1 landed the merged-cells core API (merge/unmerge/query) and its
undo/redo. Phase 2 makes the rest of the model respect a merge at the two
places the functional spec calls out for this phase:

1. **Edit guard** — `UserModel::set_user_input` must reject a write to a
   *covered* (non-anchor) cell of a merged region, while a write to the anchor
   works normally.
2. **Clear-all unmerge** — `UserModel::range_clear_all` must unmerge any region
   **fully contained** in the cleared area, bundled into its existing diff list
   so one undo restores both the cleared content and the merge. A region only
   **partially** overlapped is left intact. `range_clear_contents` and
   `range_clear_formatting` stay merge-preserving (no change).

Structural displacement (Phase 3) and bindings (Phase 4) are out of scope.

## Steps

1. **`base/src/user_model/common.rs` — imports.**
   - Add `MergeCell` to the `crate::types::{...}` import.
   - Add `number_to_column` to the `crate::expressions::utils::{...}` import.

2. **Edit guard in `set_user_input` (`common.rs:398`).**
   After the existing `is_valid_column_number` / `is_valid_row` checks and
   before reading `old_value`, add:
   ```rust
   if let Some(region) = self.model.workbook.worksheet(sheet)?.merge_at(row, column) {
       if region.row != row || region.column != column {
           let anchor = number_to_column(region.column)
               .map(|col| format!("{col}{}", region.row))
               .unwrap_or_default();
           return Err(format!(
               "cannot edit a cell inside a merged region; edit the anchor at {anchor}"
           ));
       }
   }
   ```
   `merge_at` returns an owned `Option<MergeCell>`, so there is no borrow
   conflict with the later `worksheet(sheet)?` reads. Only the public
   `UserModel::set_user_input` is guarded; internal writers go through
   `self.model.set_user_input` (base `Model`) and are intentionally unaffected
   (undo/redo, autofill, clipboard).

3. **Clear-all unmerge in `range_clear_all` (`common.rs:656`).**
   - Change `let diff_list = vec![Diff::RangeClearAll { .. }];` to `let mut
     diff_list = ...`.
   - After it, collect the regions fully contained in `range`, remove each, and
     push a `Diff::UnmergeCells` per removed region into `diff_list`:
     ```rust
     let contained: Vec<MergeCell> = self
         .model
         .workbook
         .worksheet(sheet)?
         .merge_cells_parsed()
         .into_iter()
         .filter(|region| area_contains_region(range, region))
         .collect();
     for region in contained {
         self.model.workbook.worksheet_mut(sheet)?
             .remove_merge_at(region.row, region.column);
         diff_list.push(Diff::UnmergeCells {
             sheet, row: region.row, column: region.column,
             width: region.width, height: region.height,
         });
     }
     ```
   - Add a module-level helper `area_contains_region(area: &Area, region:
     &MergeCell) -> bool` (region entirely within area).
   - Ordering is correct with the existing undo/redo: redo applies forward
     (`RangeClearAll` re-clears, then `UnmergeCells` re-removes); undo applies in
     reverse (`UnmergeCells` re-merges — clearing already-empty covered cells —
     then `RangeClearAll` restores anchor content/style). Covered-cell slots in
     `old_value` are `None`, so the restore never repopulates a covered cell.

4. **Leave `range_clear_contents` / `range_clear_formatting` untouched.**

## Tests (in `base/src/test/user_model/test_merge_cells.rs`)

- `edit_covered_cell_rejected` — merge B2:D4, `set_user_input` on a covered
  cell (C3 and B3) returns `Err` with the anchor A1 in the message; the covered
  cell stays empty.
- `edit_anchor_allowed` — `set_user_input` on the anchor of a merged region
  succeeds and stores the value.
- `edit_unmerged_cell_allowed` — a normal write to a non-merged cell still
  works (guard is inert without a merge).
- `clear_all_unmerges_contained_region` — merge fully inside the cleared area is
  removed and anchor content cleared; undo restores the merge **and** the anchor
  content; redo removes it again.
- `clear_all_leaves_partially_overlapped_region` — a region only partially
  covered by the cleared area survives (still one region).
- `clear_contents_preserves_merge` — `range_clear_contents` over a region clears
  the anchor content but keeps the merge.
- `clear_formatting_preserves_merge` — `range_clear_formatting` over a region
  keeps the merge.

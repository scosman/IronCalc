use crate::types::MergeCell;

use super::{common::UserModel, history::Diff};

impl<'a> UserModel<'a> {
    /// Merges the rectangle anchored at `(row, column)` spanning `width` columns
    /// and `height` rows into a single region, keeping only the anchor value and
    /// clearing the covered cells' content. Undoable as a single history entry
    /// that restores the discarded content.
    ///
    /// See also:
    /// * [`crate::Model::merge_cells`]
    pub fn merge_cells(
        &mut self,
        sheet: u32,
        row: i32,
        column: i32,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let old_covered = self.model.merge_cells(sheet, row, column, width, height)?;
        self.push_diff_list(vec![Diff::MergeCells {
            sheet,
            row,
            column,
            width,
            height,
            old_covered,
        }]);
        self.evaluate_if_not_paused();
        Ok(())
    }

    /// Removes the merged region containing `(row, column)` (anchor or any
    /// covered cell). A no-op if the cell is not merged. Undoable/redoable.
    ///
    /// See also:
    /// * [`crate::Model::unmerge_cells`]
    pub fn unmerge_cells(&mut self, sheet: u32, row: i32, column: i32) -> Result<(), String> {
        if let Some(region) = self.model.unmerge_cells(sheet, row, column)? {
            // Record the removed region by its anchor so undo/redo are
            // unambiguous even when the caller passed a covered cell.
            self.push_diff_list(vec![Diff::UnmergeCells {
                sheet,
                row: region.row,
                column: region.column,
                width: region.width,
                height: region.height,
            }]);
            self.evaluate_if_not_paused();
        }
        Ok(())
    }

    /// Returns every merged region on `sheet`.
    ///
    /// See also:
    /// * [`crate::Model::get_merge_cells`]
    pub fn get_merge_cells(&self, sheet: u32) -> Result<Vec<MergeCell>, String> {
        self.model.get_merge_cells(sheet)
    }

    /// Returns the merged region covering `(row, column)` (anchor or covered), or
    /// `None` if the cell is not merged.
    ///
    /// See also:
    /// * [`crate::Model::get_merge_cell`]
    pub fn get_merge_cell(
        &self,
        sheet: u32,
        row: i32,
        column: i32,
    ) -> Result<Option<MergeCell>, String> {
        self.model.get_merge_cell(sheet, row, column)
    }
}

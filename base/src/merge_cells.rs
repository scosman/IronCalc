use crate::{
    constants::{LAST_COLUMN, LAST_ROW},
    model::CellStructure,
    types::{Cell, MergeCell},
    Model,
};

/// Returns true if the two regions share at least one cell.
fn regions_intersect(a: &MergeCell, b: &MergeCell) -> bool {
    let a_row_end = a.row + a.height - 1;
    let a_column_end = a.column + a.width - 1;
    let b_row_end = b.row + b.height - 1;
    let b_column_end = b.column + b.width - 1;
    a.row <= b_row_end && b.row <= a_row_end && a.column <= b_column_end && b.column <= a_column_end
}

impl<'a> Model<'a> {
    /// Merges the rectangle anchored at `(row, column)` spanning `width` columns
    /// and `height` rows into a single region.
    ///
    /// Validation runs before any mutation, so a rejected merge leaves the
    /// workbook untouched. In order: the sheet must exist, the rectangle must be
    /// in bounds, its dimensions must be non-degenerate (`width >= 1`,
    /// `height >= 1`, and not `1x1`), it must not overlap an existing region, and
    /// it must not cover an array-formula anchor or spill cell.
    ///
    /// On success the anchor value is kept while every covered cell has its
    /// content cleared (its style is preserved). The discarded content is
    /// returned row-major over the rectangle (the anchor slot is `None`) so the
    /// caller can build an undo entry.
    pub fn merge_cells(
        &mut self,
        sheet: u32,
        row: i32,
        column: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<Vec<Option<Cell>>>, String> {
        // Validation (no mutation until every check passes).
        let worksheet = self.workbook.worksheet(sheet)?;

        // Bounds. Computed in i64 so extreme dimensions cannot overflow i32.
        let last_row = row as i64 + height as i64 - 1;
        let last_column = column as i64 + width as i64 - 1;
        if row < 1 || column < 1 || last_row > LAST_ROW as i64 || last_column > LAST_COLUMN as i64 {
            return Err("Merge range is out of bounds".to_string());
        }

        // Degenerate dimensions.
        if width < 1 || height < 1 {
            return Err("Merge width and height must be at least 1".to_string());
        }
        if width == 1 && height == 1 {
            return Err("Cannot merge a single cell".to_string());
        }

        let new_region = MergeCell {
            row,
            column,
            width,
            height,
        };

        // Overlap with an existing region.
        if worksheet
            .merge_cells_parsed()
            .iter()
            .any(|existing| regions_intersect(existing, &new_region))
        {
            return Err("Merge range overlaps an existing merged region".to_string());
        }

        let row_end = row + height;
        let column_end = column + width;

        // Array-formula / spill collision. Mirror the array-split guards
        // (`can_insert_rows` et al. in `actions.rs`): derive each array formula's
        // occupied rectangle from its anchor's declared spill range via
        // `get_cell_structure`, so detection is independent of whether the spill
        // has been materialized (an anchor whose spill has not yet been evaluated
        // is still caught). Unlike those guards, a merge is rejected on *any*
        // intersection — CSE or dynamic — matching the merge spec.
        let anchor_coords: Vec<(i32, i32)> = worksheet
            .sheet_data
            .iter()
            .flat_map(|(r, row_data)| row_data.keys().map(move |c| (*r, *c)))
            .collect();
        for (r, c) in anchor_coords {
            let (aw, ah) = match self.get_cell_structure(sheet, r, c)? {
                CellStructure::ArrayFormula { range } | CellStructure::DynamicFormula { range } => {
                    range
                }
                // Spill cells are covered by their anchor's range above.
                _ => continue,
            };
            let occupied = MergeCell {
                row: r,
                column: c,
                width: aw,
                height: ah,
            };
            if regions_intersect(&occupied, &new_region) {
                return Err("Cannot merge cells that contain an array formula".to_string());
            }
        }

        // Mutation. Record the covered content (anchor excluded) before clearing.
        // `old_covered` is a dense `Option<Cell>` slot per cell in the rectangle
        // (anchor slot `None`); this trades some undo-history memory for simple,
        // index-aligned restoration. Fine given merges are expected to be small.
        let worksheet = self.workbook.worksheet_mut(sheet)?;
        let mut old_covered = Vec::with_capacity(height as usize);
        for r in row..row_end {
            let mut row_content = Vec::with_capacity(width as usize);
            for c in column..column_end {
                if r == row && c == column {
                    row_content.push(None);
                } else {
                    row_content.push(worksheet.cell(r, c).cloned());
                }
            }
            old_covered.push(row_content);
        }
        worksheet.apply_merge(&new_region);
        Ok(old_covered)
    }

    /// Removes the merged region containing `(row, column)` (anchor or any
    /// covered cell) and returns it. If the cell is not part of any region this
    /// is a no-op returning `Ok(None)`. Covered cells are already empty, so no
    /// content is synthesized.
    pub fn unmerge_cells(
        &mut self,
        sheet: u32,
        row: i32,
        column: i32,
    ) -> Result<Option<MergeCell>, String> {
        let worksheet = self.workbook.worksheet_mut(sheet)?;
        Ok(worksheet.remove_merge_at(row, column))
    }

    /// Returns every merged region on `sheet` as normalized [`MergeCell`]s.
    pub fn get_merge_cells(&self, sheet: u32) -> Result<Vec<MergeCell>, String> {
        Ok(self.workbook.worksheet(sheet)?.merge_cells_parsed())
    }

    /// Returns the merged region covering `(row, column)` (anchor or covered),
    /// or `None` if the cell is not merged.
    pub fn get_merge_cell(
        &self,
        sheet: u32,
        row: i32,
        column: i32,
    ) -> Result<Option<MergeCell>, String> {
        Ok(self.workbook.worksheet(sheet)?.merge_at(row, column))
    }
}

#![allow(clippy::unwrap_used)]

use crate::{
    expressions::types::Area, test::user_model::util::new_empty_user_model, types::MergeCell,
};

fn cell(model: &crate::UserModel, row: i32, column: i32) -> String {
    model.get_formatted_cell_value(0, row, column).unwrap()
}

#[test]
fn merge_reflected_in_queries() {
    let mut model = new_empty_user_model();
    // Merge B2:D4 -> anchor (2, 2), 3 columns wide, 3 rows tall.
    model.merge_cells(0, 2, 2, 3, 3).unwrap();

    assert_eq!(
        model.get_merge_cells(0).unwrap(),
        vec![MergeCell {
            row: 2,
            column: 2,
            width: 3,
            height: 3,
        }]
    );

    let expected = Some(MergeCell {
        row: 2,
        column: 2,
        width: 3,
        height: 3,
    });
    // Anchor resolves to the region.
    assert_eq!(model.get_merge_cell(0, 2, 2).unwrap(), expected);
    // Every covered cell resolves to the region.
    assert_eq!(model.get_merge_cell(0, 4, 4).unwrap(), expected);
    assert_eq!(model.get_merge_cell(0, 3, 3).unwrap(), expected);
    // Cells outside the region resolve to None.
    assert_eq!(model.get_merge_cell(0, 1, 1).unwrap(), None);
    assert_eq!(model.get_merge_cell(0, 5, 5).unwrap(), None);
    assert_eq!(model.get_merge_cell(0, 2, 5).unwrap(), None);
}

#[test]
fn merge_clears_covered_keeps_anchor() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "anchor").unwrap();
    model.set_user_input(0, 1, 2, "right").unwrap();
    model.set_user_input(0, 2, 1, "down").unwrap();
    model.set_user_input(0, 2, 2, "diag").unwrap();

    model.merge_cells(0, 1, 1, 2, 2).unwrap();

    assert_eq!(cell(&model, 1, 1), "anchor");
    assert_eq!(cell(&model, 1, 2), "");
    assert_eq!(cell(&model, 2, 1), "");
    assert_eq!(cell(&model, 2, 2), "");
}

#[test]
fn merge_preserves_covered_cell_style_through_undo() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 2, "right").unwrap();
    // Bold the covered cell B1.
    model
        .update_range_style(
            &Area {
                sheet: 0,
                row: 1,
                column: 2,
                width: 1,
                height: 1,
            },
            "font.b",
            "true",
        )
        .unwrap();
    assert!(model.get_cell_style(0, 1, 2).unwrap().font.b);

    // Merge A1:B1 -> B1 becomes covered; content is cleared but style is kept.
    model.merge_cells(0, 1, 1, 2, 1).unwrap();
    assert_eq!(cell(&model, 1, 2), "");
    assert!(
        model.get_cell_style(0, 1, 2).unwrap().font.b,
        "covered cell must keep its style after merge"
    );

    // Undo restores both the content and the style.
    model.undo().unwrap();
    assert_eq!(cell(&model, 1, 2), "right");
    assert!(
        model.get_cell_style(0, 1, 2).unwrap().font.b,
        "covered cell style must be intact after undo"
    );
}

#[test]
fn merge_undo_redo() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "anchor").unwrap();
    model.set_user_input(0, 1, 2, "right").unwrap();
    model.set_user_input(0, 2, 1, "down").unwrap();
    model.set_user_input(0, 2, 2, "diag").unwrap();

    model.merge_cells(0, 1, 1, 2, 2).unwrap();
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);

    // Undo restores the covered content and drops the region.
    model.undo().unwrap();
    assert!(model.get_merge_cells(0).unwrap().is_empty());
    assert_eq!(cell(&model, 1, 1), "anchor");
    assert_eq!(cell(&model, 1, 2), "right");
    assert_eq!(cell(&model, 2, 1), "down");
    assert_eq!(cell(&model, 2, 2), "diag");

    // Redo re-applies the merge (region back, covered cleared).
    model.redo().unwrap();
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
    assert_eq!(cell(&model, 1, 1), "anchor");
    assert_eq!(cell(&model, 1, 2), "");
    assert_eq!(cell(&model, 2, 1), "");
    assert_eq!(cell(&model, 2, 2), "");
}

#[test]
fn unmerge_removes_region_via_covered_cell() {
    let mut model = new_empty_user_model();
    model.merge_cells(0, 1, 1, 2, 2).unwrap();
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);

    // Unmerge addressing a covered (non-anchor) cell.
    model.unmerge_cells(0, 2, 2).unwrap();
    assert!(model.get_merge_cells(0).unwrap().is_empty());

    // Undo restores the merge.
    model.undo().unwrap();
    assert_eq!(
        model.get_merge_cells(0).unwrap(),
        vec![MergeCell {
            row: 1,
            column: 1,
            width: 2,
            height: 2,
        }]
    );

    // Redo removes it again.
    model.redo().unwrap();
    assert!(model.get_merge_cells(0).unwrap().is_empty());
}

#[test]
fn unmerge_noop_when_not_merged() {
    let mut model = new_empty_user_model();
    // No region contains (1, 1); the call succeeds and records no history.
    model.unmerge_cells(0, 1, 1).unwrap();
    assert!(!model.can_undo());
    assert!(model.get_merge_cells(0).unwrap().is_empty());
}

#[test]
fn validation_out_of_bounds() {
    let mut model = new_empty_user_model();
    // width extends past the last column.
    assert!(model.merge_cells(0, 1, 1, 20_000, 1).is_err());
    assert!(model.get_merge_cells(0).unwrap().is_empty());
}

#[test]
fn validation_degenerate_1x1() {
    let mut model = new_empty_user_model();
    assert!(model.merge_cells(0, 2, 2, 1, 1).is_err());
    assert!(model.get_merge_cells(0).unwrap().is_empty());
}

#[test]
fn validation_zero_or_negative_dims() {
    let mut model = new_empty_user_model();
    assert!(model.merge_cells(0, 2, 2, 0, 3).is_err());
    assert!(model.merge_cells(0, 2, 2, 3, 0).is_err());
    assert!(model.merge_cells(0, 2, 2, -1, 3).is_err());
    assert!(model.merge_cells(0, 2, 2, 3, -1).is_err());
    assert!(model.get_merge_cells(0).unwrap().is_empty());
}

#[test]
fn validation_overlap_leaves_workbook_unchanged() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 2, 3, "keep-c2").unwrap();
    model.set_user_input(0, 3, 3, "keep-c3").unwrap();
    model.merge_cells(0, 1, 1, 2, 2).unwrap();

    // B2:C3 overlaps the existing A1:B2 region at B2 (partial/corner overlap).
    assert!(model.merge_cells(0, 2, 2, 2, 2).is_err());
    // An exact-duplicate merge of the existing region is also rejected.
    assert!(model.merge_cells(0, 1, 1, 2, 2).is_err());
    // A fully-nested smaller region (A1:A2 inside A1:B2) is rejected too.
    assert!(model.merge_cells(0, 1, 1, 1, 2).is_err());

    // Only the original region remains and the would-be covered content is intact.
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
    assert_eq!(cell(&model, 2, 3), "keep-c2");
    assert_eq!(cell(&model, 3, 3), "keep-c3");
}

#[test]
fn validation_array_spill_collision() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 2, "10").unwrap();
    model.set_user_input(0, 2, 2, "20").unwrap();
    model.set_user_input(0, 3, 2, "30").unwrap();
    // Dynamic array in A1 spilling A1:A3 (A1 anchor, A2/A3 spill cells).
    model.set_user_input(0, 1, 1, "=B1:B3").unwrap();

    // A merge covering the anchor is rejected.
    assert!(model.merge_cells(0, 1, 1, 1, 2).is_err());
    // A merge covering only spill cells is rejected.
    assert!(model.merge_cells(0, 2, 1, 1, 2).is_err());
    assert!(model.get_merge_cells(0).unwrap().is_empty());

    // A merge clear of the array is allowed.
    model.merge_cells(0, 1, 4, 2, 2).unwrap();
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
}

#[test]
fn validation_invalid_sheet() {
    let mut model = new_empty_user_model();
    assert!(model.merge_cells(5, 1, 1, 2, 2).is_err());
    assert!(model.unmerge_cells(5, 1, 1).is_err());
    assert!(model.get_merge_cells(5).is_err());
    assert!(model.get_merge_cell(5, 1, 1).is_err());
}

fn area(row: i32, column: i32, width: i32, height: i32) -> Area {
    Area {
        sheet: 0,
        row,
        column,
        width,
        height,
    }
}

#[test]
fn edit_covered_cell_rejected() {
    let mut model = new_empty_user_model();
    // Merge B2:D4 -> anchor (2, 2).
    model.merge_cells(0, 2, 2, 3, 3).unwrap();

    // Writing to a covered interior cell is rejected and names the anchor.
    let err = model.set_user_input(0, 3, 3, "nope").unwrap_err();
    assert_eq!(
        err,
        "cannot edit a cell inside a merged region; edit the anchor at B2"
    );
    // A covered cell on the region's edge is rejected too.
    assert!(model.set_user_input(0, 2, 4, "nope").is_err());
    assert_eq!(cell(&model, 2, 4), "");

    // The covered cell stays empty and the failed writes recorded no history:
    // undoing once removes the merge (the only prior operation), leaving nothing
    // to undo.
    assert_eq!(cell(&model, 3, 3), "");
    model.undo().unwrap();
    assert!(model.get_merge_cells(0).unwrap().is_empty());
    assert!(!model.can_undo());
}

#[test]
fn internal_writers_bypass_edit_guard() {
    let mut model = new_empty_user_model();
    // Merge B2:D4 -> covered cells C2/C3/C4 etc. are interior.
    model.merge_cells(0, 2, 2, 3, 3).unwrap();

    // Undo/redo route through the base `Model::set_user_input`, not the guarded
    // `UserModel::set_user_input`. A normal edit to a non-merged cell must still
    // round-trip through history with a merge present.
    model.set_user_input(0, 1, 6, "free").unwrap(); // F1
    model.undo().unwrap();
    assert_eq!(cell(&model, 1, 6), "");
    model.redo().unwrap();
    assert_eq!(cell(&model, 1, 6), "free");

    // Autofill also writes through the base writer. Filling a series from C1
    // downward into C2/C3/C4 (all covered by the merge) must not be rejected by
    // the covered-cell guard.
    model.set_user_input(0, 1, 3, "seed").unwrap(); // C1, above the merge
    model.auto_fill_rows(&area(1, 3, 1, 1), 4).unwrap();
    assert_eq!(cell(&model, 2, 3), "seed");
    assert_eq!(cell(&model, 3, 3), "seed");
    // The merge itself is untouched by these internal writes.
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
}

#[test]
fn edit_anchor_allowed() {
    let mut model = new_empty_user_model();
    model.merge_cells(0, 2, 2, 3, 3).unwrap();

    // Writing to the anchor works normally.
    model.set_user_input(0, 2, 2, "anchor value").unwrap();
    assert_eq!(cell(&model, 2, 2), "anchor value");
}

#[test]
fn edit_unmerged_cell_allowed() {
    let mut model = new_empty_user_model();
    // With no merge in play the guard is inert.
    model.set_user_input(0, 3, 3, "free").unwrap();
    assert_eq!(cell(&model, 3, 3), "free");
}

#[test]
fn clear_all_unmerges_contained_region() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 2, 2, "anchor").unwrap();
    // Merge B2:C3, fully inside the area we will clear (A1:D4).
    model.merge_cells(0, 2, 2, 2, 2).unwrap();
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);

    model.range_clear_all(&area(1, 1, 4, 4)).unwrap();
    // The region is unmerged and the anchor content is cleared.
    assert!(model.get_merge_cells(0).unwrap().is_empty());
    assert_eq!(cell(&model, 2, 2), "");

    // A single undo restores both the merge and the anchor content.
    model.undo().unwrap();
    assert_eq!(
        model.get_merge_cells(0).unwrap(),
        vec![MergeCell {
            row: 2,
            column: 2,
            width: 2,
            height: 2,
        }]
    );
    assert_eq!(cell(&model, 2, 2), "anchor");

    // Redo clears and unmerges again.
    model.redo().unwrap();
    assert!(model.get_merge_cells(0).unwrap().is_empty());
    assert_eq!(cell(&model, 2, 2), "");
}

#[test]
fn clear_all_leaves_partially_overlapped_region() {
    let mut model = new_empty_user_model();
    // Merge B2:D4.
    model.merge_cells(0, 2, 2, 3, 3).unwrap();

    // Clear A1:C3 — overlaps the region but does not fully contain it.
    model.range_clear_all(&area(1, 1, 3, 3)).unwrap();

    // The region survives intact.
    assert_eq!(
        model.get_merge_cells(0).unwrap(),
        vec![MergeCell {
            row: 2,
            column: 2,
            width: 3,
            height: 3,
        }]
    );
}

#[test]
fn clear_contents_preserves_merge() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 2, 2, "anchor").unwrap();
    model.merge_cells(0, 2, 2, 2, 2).unwrap();

    model.range_clear_contents(&area(1, 1, 4, 4)).unwrap();
    // Anchor content is cleared but the merge is preserved.
    assert_eq!(cell(&model, 2, 2), "");
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
}

#[test]
fn clear_formatting_preserves_merge() {
    let mut model = new_empty_user_model();
    model.merge_cells(0, 2, 2, 2, 2).unwrap();

    model.range_clear_formatting(&area(1, 1, 4, 4)).unwrap();
    // Formatting clear never touches merges.
    assert_eq!(model.get_merge_cells(0).unwrap().len(), 1);
}

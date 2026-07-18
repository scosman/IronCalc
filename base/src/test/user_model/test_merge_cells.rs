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

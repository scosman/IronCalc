#![allow(clippy::unwrap_used)]

//! Interaction tests between cell links and merged regions.
//!
//! Row/column move and range clear each hook both features in the same
//! functions, so their behaviours can drift apart without any compiler or merge
//! signal. These tests pin the combined behaviour.

use crate::expressions::types::Area;
use crate::test::user_model::util::new_empty_user_model;
use crate::types::{Link, MergeCell};
use crate::UserModel;

const LINKED: &str = "https://www.ironcalc.com/";
/// A URL-shaped value whose link the user removed: the cell text alone is enough
/// for a move rebuild to auto-link it again, which must not happen.
const UNLINKED_URL_TEXT: &str = "https://autolink.example.com/";

fn external(target: &str) -> Link {
    Link::External {
        target: target.to_string(),
        tooltip: None,
    }
}

fn merges(model: &UserModel) -> Vec<MergeCell> {
    model.get_merge_cells(0).unwrap()
}

fn merge_cell(row: i32, column: i32, width: i32, height: i32) -> MergeCell {
    MergeCell {
        row,
        column,
        width,
        height,
    }
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

/// B2:C3 merged with a link on its anchor, plus a URL-shaped but deliberately
/// unlinked cell at `(stray_row, stray_column)` outside the region.
fn model_with_merged_link(stray_row: i32, stray_column: i32) -> UserModel<'static> {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 2, 2, "anchor").unwrap();
    model.merge_cells(0, 2, 2, 2, 2).unwrap();
    model
        .set_cell_link(0, 2, 2, external(LINKED), None)
        .unwrap();

    model
        .set_user_input(0, stray_row, stray_column, UNLINKED_URL_TEXT)
        .unwrap();
    model.delete_cell_link(0, stray_row, stray_column).unwrap();
    assert_eq!(model.get_cell_link(0, stray_row, stray_column), Ok(None));

    model
}

#[test]
fn move_rows_carries_merged_region_and_link() {
    // The stray URL text shares the moved rows but sits outside B2:C3.
    let mut model = model_with_merged_link(2, 4);

    // Move rows 2..3 down by 3 -> rows 5..6.
    model.move_rows_action(0, 2, 2, 3).unwrap();

    assert_eq!(merges(&model), vec![merge_cell(5, 2, 2, 2)]);
    assert_eq!(
        model.get_cell_link(0, 5, 2),
        Ok(Some(external(LINKED))),
        "the link must land on the moved region's anchor"
    );
    assert_eq!(
        model.get_cell_link(0, 2, 2),
        Ok(None),
        "the source anchor must keep no link"
    );
    assert_eq!(
        model.get_formatted_cell_value(0, 5, 4).unwrap(),
        UNLINKED_URL_TEXT
    );
    assert_eq!(
        model.get_cell_link(0, 5, 4),
        Ok(None),
        "the rebuild must not auto-link a value the user had unlinked"
    );

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 5, 2), Ok(None));
    assert_eq!(model.get_cell_link(0, 2, 4), Ok(None));

    model.redo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(5, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 5, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 5, 4), Ok(None));
}

#[test]
fn move_columns_carries_merged_region_and_link() {
    // The stray URL text shares the moved columns but sits outside B2:C3.
    let mut model = model_with_merged_link(4, 2);

    // Move columns 2..3 right by 3 -> columns 5..6.
    model.move_columns_action(0, 2, 2, 3).unwrap();

    assert_eq!(merges(&model), vec![merge_cell(2, 5, 2, 2)]);
    assert_eq!(
        model.get_cell_link(0, 2, 5),
        Ok(Some(external(LINKED))),
        "the link must land on the moved region's anchor"
    );
    assert_eq!(
        model.get_cell_link(0, 2, 2),
        Ok(None),
        "the source anchor must keep no link"
    );
    assert_eq!(
        model.get_formatted_cell_value(0, 4, 5).unwrap(),
        UNLINKED_URL_TEXT
    );
    assert_eq!(
        model.get_cell_link(0, 4, 5),
        Ok(None),
        "the rebuild must not auto-link a value the user had unlinked"
    );

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 2, 5), Ok(None));
    assert_eq!(model.get_cell_link(0, 4, 2), Ok(None));

    model.redo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 5, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 5), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 4, 5), Ok(None));
}

#[test]
fn range_clear_all_drops_link_and_merge_in_one_undo_step() {
    let mut model = model_with_merged_link(6, 6);

    // A1:D4 fully contains the merged region.
    model.range_clear_all(&area(1, 1, 4, 4)).unwrap();

    assert!(merges(&model).is_empty());
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "");

    // One undo restores content, link and merge together.
    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "anchor");

    model.redo().unwrap();
    assert!(merges(&model).is_empty());
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));
}

#[test]
fn range_clear_contents_drops_link_but_keeps_merge() {
    let mut model = model_with_merged_link(6, 6);

    model.range_clear_contents(&area(1, 1, 4, 4)).unwrap();

    assert_eq!(
        merges(&model),
        vec![merge_cell(2, 2, 2, 2)],
        "clearing contents must not unmerge"
    );
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "");

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "anchor");
}

#[test]
fn range_clear_leaves_links_outside_the_range_alone() {
    let mut model = model_with_merged_link(6, 6);
    model
        .set_cell_link(0, 10, 10, external("https://outside.example.com/"), None)
        .unwrap();

    model.range_clear_all(&area(1, 1, 4, 4)).unwrap();

    assert_eq!(
        model.get_cell_link(0, 10, 10),
        Ok(Some(external("https://outside.example.com/")))
    );
}

// --- Structural delete -------------------------------------------------------
//
// `delete_rows` / `delete_columns` are where the sync's only hand-resolved hunks
// live: upstream seeds `diff_list` from `range_link_diffs`, our work adds
// `old_merge_cells` / `old_frozen_*` to the same `Diff`. Both halves must survive.

#[test]
fn delete_rows_over_a_merged_region_drops_link_and_merge() {
    let mut model = model_with_merged_link(10, 10);

    // Rows 2..3 are exactly the merged B2:C3.
    model.delete_rows(0, 2, 2).unwrap();
    assert!(merges(&model).is_empty());
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));

    // One undo restores content, link and merge together.
    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "anchor");
}

#[test]
fn delete_rows_above_shifts_the_merged_region_with_its_link() {
    let mut model = model_with_merged_link(10, 10);

    // Row 1 is above the region: both the merge and the link shift up by one.
    model.delete_rows(0, 1, 1).unwrap();
    assert_eq!(merges(&model), vec![merge_cell(1, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 1, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
}

#[test]
fn delete_columns_over_a_merged_region_drops_link_and_merge() {
    let mut model = model_with_merged_link(10, 10);

    // Columns 2..3 are exactly the merged B2:C3.
    model.delete_columns(0, 2, 2).unwrap();
    assert!(merges(&model).is_empty());
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "anchor");
}

#[test]
fn delete_columns_before_shifts_the_merged_region_with_its_link() {
    let mut model = model_with_merged_link(10, 10);

    // Column 1 is left of the region: both the merge and the link shift left by one.
    model.delete_columns(0, 1, 1).unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 1, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 1), Ok(Some(external(LINKED))));
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(None));

    model.undo().unwrap();
    assert_eq!(merges(&model), vec![merge_cell(2, 2, 2, 2)]);
    assert_eq!(model.get_cell_link(0, 2, 2), Ok(Some(external(LINKED))));
}

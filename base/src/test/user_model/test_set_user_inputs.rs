#![allow(clippy::unwrap_used)]

use crate::test::user_model::util::new_empty_user_model;
use crate::types::{Color, Link};
use crate::user_model::history::{Diff, QueueDiffs};
use crate::UserModel;

const URL: &str = "https://www.ironcalc.com/";
const OTHER_URL: &str = "https://docs.ironcalc.com/";
const THIRD_URL: &str = "https://third.example/";

fn external(target: &str) -> Link {
    Link::External {
        target: target.to_string(),
        tooltip: None,
    }
}

/// Drains the send queue and names, in order, the diffs of the single operation it
/// holds. Modelled on `last_diff_list` in `test_batch_row_column_diff.rs`, but it
/// asserts the queue holds exactly one entry so a stale undo entry cannot silently
/// merge into the result.
///
/// Cell state cannot see a *spurious* `SetCellStyle` diff when the cell pre-existed:
/// its `SetCellValue` snapshot carries the whole `Cell`, style index included, so undo
/// reconstructs the style from the value diff alone. (A *missing* style diff is visible
/// in cell state whenever the cell was empty pre-batch.) The recorded diff list is the
/// only place both directions are observable.
fn recorded_diffs(model: &mut UserModel) -> Vec<&'static str> {
    let queue = bitcode::decode::<Vec<QueueDiffs>>(&model.flush_send_queue()).unwrap();
    let [operation] = &queue[..] else {
        panic!(
            "expected exactly one operation in the send queue, got {}",
            queue.len()
        );
    };
    operation
        .list
        .iter()
        .map(|diff| match diff {
            Diff::SetCellValue { .. } => "SetCellValue",
            Diff::SetCellStyle { .. } => "SetCellStyle",
            Diff::SetCellLink { .. } => "SetCellLink",
            _ => "other",
        })
        .collect()
}

#[test]
fn batch_write_is_one_undo_step() {
    let mut model = new_empty_user_model();
    model
        .set_user_inputs(&[
            (0, 1, 1, "cat".to_string()),  // A1
            (0, 1, 2, "dog".to_string()),  // B1
            (0, 3, 5, "bird".to_string()), // E3 (scattered, non-contiguous)
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "cat");
    assert_eq!(model.get_cell_content(0, 1, 2).unwrap(), "dog");
    assert_eq!(model.get_cell_content(0, 3, 5).unwrap(), "bird");

    // The defining property: a SINGLE undo reverts the whole batch, and it is the
    // only history entry.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "");
    assert_eq!(model.get_cell_content(0, 1, 2).unwrap(), "");
    assert_eq!(model.get_cell_content(0, 3, 5).unwrap(), "");
    assert!(!model.can_undo(), "the batch is a single history entry");

    // A SINGLE redo re-applies the whole batch.
    model.redo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "cat");
    assert_eq!(model.get_cell_content(0, 1, 2).unwrap(), "dog");
    assert_eq!(model.get_cell_content(0, 3, 5).unwrap(), "bird");
}

#[test]
fn batch_overwrite_undo_restores_prior_values() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "old-a").unwrap();
    model.set_user_input(0, 2, 2, "old-b").unwrap();

    model
        .set_user_inputs(&[
            (0, 1, 1, "new-a".to_string()),
            (0, 2, 2, "new-b".to_string()),
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "new-a");
    assert_eq!(model.get_cell_content(0, 2, 2).unwrap(), "new-b");

    // A single undo restores BOTH prior (non-empty) values.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "old-a");
    assert_eq!(model.get_cell_content(0, 2, 2).unwrap(), "old-b");
    // The two seed edits are still individually undoable — the batch was one entry.
    assert!(model.can_undo());
}

#[test]
fn batch_recomputes_dependent_formula() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 3, "=A1+B1").unwrap(); // C1 = A1 + B1 -> 0
    assert_eq!(model.get_formatted_cell_value(0, 1, 3).unwrap(), "0");

    model
        .set_user_inputs(&[
            (0, 1, 1, "2".to_string()), // A1
            (0, 1, 2, "3".to_string()), // B1
        ])
        .unwrap();
    // One evaluate at the end of the batch reflects both inputs.
    assert_eq!(model.get_formatted_cell_value(0, 1, 3).unwrap(), "5");

    // A single undo reverts both inputs and the dependent recomputes back to 0.
    model.undo().unwrap();
    assert_eq!(model.get_formatted_cell_value(0, 1, 3).unwrap(), "0");
}

#[test]
fn empty_batch_is_a_noop() {
    let mut model = new_empty_user_model();
    model.set_user_inputs(&[]).unwrap();
    // An empty batch records no history entry.
    assert!(!model.can_undo());
}

#[test]
fn out_of_range_batch_is_rejected_without_mutating() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "seed").unwrap();

    // The second entry is out of range (row 0 is invalid — rows are 1-based).
    let result = model.set_user_inputs(&[
        (0, 2, 2, "written?".to_string()),
        (0, 0, 1, "bad".to_string()),
    ]);
    assert!(result.is_err());
    // All-or-nothing: the valid entry was NOT written, and no batch history entry
    // exists beyond the seed edit.
    assert_eq!(model.get_cell_content(0, 2, 2).unwrap(), "");
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "");
    assert!(!model.can_undo(), "only the seed edit was ever recorded");
}

#[test]
fn batch_across_sheets_is_one_undo_step() {
    let mut model = new_empty_user_model();
    model.new_sheet().unwrap(); // Sheet2 = index 1

    model
        .set_user_inputs(&[
            (0, 1, 1, "on-sheet1".to_string()),
            (1, 1, 1, "on-sheet2".to_string()),
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "on-sheet1");
    assert_eq!(model.get_cell_content(1, 1, 1).unwrap(), "on-sheet2");

    // A single undo clears both sheets' cells (one history entry spans sheets).
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "");
    assert_eq!(model.get_cell_content(1, 1, 1).unwrap(), "");
    // Only the new_sheet remains undoable.
    assert!(model.can_undo());
}

#[test]
fn batch_overwriting_a_dynamic_array_anchor_undoes_to_the_spill() {
    let mut model = new_empty_user_model();
    // Seed a dynamic array in A1 that spills into A1:A3.
    model.set_user_input(0, 1, 2, "10").unwrap(); // B1
    model.set_user_input(0, 2, 2, "20").unwrap(); // B2
    model.set_user_input(0, 3, 2, "30").unwrap(); // B3
    model.set_user_input(0, 1, 1, "=B1:B3").unwrap(); // A1 anchor
    assert_eq!(model.get_formatted_cell_value(0, 2, 1).unwrap(), "20"); // A2 is a spill cell

    // One batch overwrites the anchor (A1) — which clears the spill — AND a former
    // spill cell (A2). Both old_values must be snapshotted *before* the A1 write
    // clears the spill; otherwise a single undo could not restore the array.
    model
        .set_user_inputs(&[
            (0, 1, 1, "x".to_string()), // A1, the anchor
            (0, 2, 1, "y".to_string()), // A2, previously a spill cell
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "x");
    assert_eq!(model.get_cell_content(0, 2, 1).unwrap(), "y");
    assert_eq!(model.get_cell_content(0, 3, 1).unwrap(), ""); // the rest of the spill is gone

    // A single undo restores the dynamic array anchor and re-spills A2 and A3.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "=B1:B3");
    assert_eq!(model.get_formatted_cell_value(0, 1, 1).unwrap(), "10");
    assert_eq!(model.get_formatted_cell_value(0, 2, 1).unwrap(), "20");
    assert_eq!(model.get_formatted_cell_value(0, 3, 1).unwrap(), "30");
}

#[test]
fn batch_with_repeated_cell_keeps_last_write_and_undoes_as_one() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "orig").unwrap(); // A1

    // The same cell appears twice in the batch: the last value wins.
    model
        .set_user_inputs(&[
            (0, 1, 1, "first".to_string()),
            (0, 1, 1, "second".to_string()),
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "second");

    // A single undo reverts the whole batch back to the original value.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "orig");
    assert!(model.can_undo(), "only the seed edit remains");
}

#[test]
fn batch_propagates_as_one_diff_list() {
    let mut model = new_empty_user_model();
    model
        .set_user_inputs(&[(0, 1, 1, "a".to_string()), (0, 2, 2, "b".to_string())])
        .unwrap();
    let send_queue = model.flush_send_queue();

    // A fresh peer replays the batch as one external diff list.
    let mut peer = new_empty_user_model();
    peer.apply_external_diffs(&send_queue).unwrap();
    assert_eq!(peer.get_cell_content(0, 1, 1).unwrap(), "a");
    assert_eq!(peer.get_cell_content(0, 2, 2).unwrap(), "b");
}

#[test]
fn mid_batch_write_failure_rolls_back_the_writes_already_made() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "seed A1").unwrap();
    model.set_user_input(0, 1, 3, "1").unwrap(); // C1
    model.set_user_input(0, 2, 3, "2").unwrap(); // C2

    // B1:B2 is a CSE array formula: writing into it is rejected by
    // `Model::set_user_input`, which is how a batch fails *mid-write* rather than
    // during the up-front coordinate validation.
    model
        .set_user_array_formula(0, 1, 2, 1, 2, "=C1:C2")
        .unwrap();

    let result = model.set_user_inputs(&[
        (0, 1, 1, "written".to_string()), // A1 — succeeds, then must be rolled back
        (0, 1, 2, "boom".to_string()),    // B1 — part of the array formula, fails
        (0, 5, 5, "never".to_string()),   // E5 — never attempted
    ]);
    assert!(result.is_err(), "writing into an array formula must fail");

    // The successful write is undone and the untouched cell was never written.
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "seed A1");
    assert_eq!(model.get_cell_content(0, 5, 5).unwrap(), "");
    // The array formula survived intact.
    assert_eq!(model.get_formatted_cell_value(0, 1, 2).unwrap(), "1");
    assert_eq!(model.get_formatted_cell_value(0, 2, 2).unwrap(), "2");

    // No history entry was recorded for the rejected batch: undoing walks back to
    // the array formula, then the seed edits.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 2).unwrap(), "");
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 2, 3).unwrap(), "");
}

#[test]
fn batch_auto_link_is_undone_with_the_batch() {
    let mut model = new_empty_user_model();

    model
        .set_user_inputs(&[
            (0, 1, 1, URL.to_string()),     // A1 — URL-shaped, auto-linked
            (0, 1, 2, "plain".to_string()), // B1 — not a link
        ])
        .unwrap();
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));
    assert_eq!(model.get_cell_link(0, 1, 2).unwrap(), None);
    // a newly auto-created link also styles the cell
    let style = model.get_cell_style(0, 1, 1).unwrap();
    assert!(style.font.u);
    assert_eq!(style.font.color, Color::Theme(10, 0.0));

    // One undo reverts the value AND the link AND the link styling.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "");
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), None);
    let style = model.get_cell_style(0, 1, 1).unwrap();
    assert!(!style.font.u);
    assert_eq!(style.font.color, Color::None);
    assert!(!model.can_undo(), "the batch is a single history entry");

    // A single redo puts value, link and styling back.
    model.redo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), URL);
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));
    let style = model.get_cell_style(0, 1, 1).unwrap();
    assert!(style.font.u);
    assert_eq!(style.font.color, Color::Theme(10, 0.0));
}

#[test]
fn batch_auto_link_undo_restores_the_previous_link() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, URL).unwrap();
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));

    model
        .set_user_inputs(&[(0, 1, 1, OTHER_URL.to_string())])
        .unwrap();
    assert_eq!(
        model.get_cell_link(0, 1, 1).unwrap(),
        Some(external(OTHER_URL))
    );

    // One undo restores the *previous* target, not merely "no link".
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), URL);
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));
}

#[test]
fn batch_emptying_a_linked_cell_undoes_to_the_link() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, URL).unwrap();

    // An empty input clears the cell, which also drops its link.
    model.set_user_inputs(&[(0, 1, 1, String::new())]).unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "");
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), None);

    // One undo restores content and link together.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), URL);
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));
}

#[test]
fn a_peer_replaying_the_batch_and_its_undo_ends_up_link_free() {
    let mut model = new_empty_user_model();
    model
        .set_user_inputs(&[(0, 1, 1, URL.to_string())])
        .unwrap();
    let mut peer = new_empty_user_model();
    peer.apply_external_diffs(&model.flush_send_queue())
        .unwrap();
    assert_eq!(peer.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));

    // Replaying the batch's *undo* has to remove the link on the peer as well. The
    // peer only ever sees the recorded diffs, so this fails unless the link change is
    // in the diff list — clearing a cell's contents does not by itself drop its link.
    model.undo().unwrap();
    peer.apply_external_diffs(&model.flush_send_queue())
        .unwrap();
    assert_eq!(peer.get_cell_content(0, 1, 1).unwrap(), "");
    assert_eq!(peer.get_cell_link(0, 1, 1).unwrap(), None);
    let style = peer.get_cell_style(0, 1, 1).unwrap();
    assert!(!style.font.u);
}

/// Seeds C1:C2 and makes B1:B2 a CSE array formula. Writing into B1 is rejected by
/// `Model::set_user_input`, which is how a batch fails *mid-write* rather than during
/// the up-front coordinate validation.
fn seed_array_formula_blocking_b1(model: &mut UserModel) {
    model.set_user_input(0, 1, 3, "1").unwrap(); // C1
    model.set_user_input(0, 2, 3, "2").unwrap(); // C2
    model
        .set_user_array_formula(0, 1, 2, 1, 2, "=C1:C2")
        .unwrap();
}

#[test]
fn mid_batch_failure_rolls_back_an_auto_created_link() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, "seed A1").unwrap();
    seed_array_formula_blocking_b1(&mut model);
    let style_before = model.get_cell_style(0, 1, 1).unwrap();

    let result = model.set_user_inputs(&[
        (0, 1, 1, URL.to_string()),    // A1 — succeeds and auto-links
        (0, 1, 2, "boom".to_string()), // B1 — inside the array formula, fails
    ]);
    assert!(result.is_err(), "writing into an array formula must fail");

    // The rollback takes the link with the value, and leaves the cell looking exactly
    // as it did. (The style assertion is a plain end-state guard, not a check on the
    // style diff — see `recorded_diffs` for why cell state cannot see that.)
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "seed A1");
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), None);
    assert_eq!(model.get_cell_style(0, 1, 1).unwrap(), style_before);

    // No history entry was recorded for the rejected batch: the next undo walks back
    // past it, to the array formula, and leaves A1 where the rollback put it.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 2).unwrap(), "");
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), "seed A1");
}

#[test]
fn mid_batch_failure_restores_a_replaced_link() {
    let mut model = new_empty_user_model();
    model.set_user_input(0, 1, 1, URL).unwrap();
    seed_array_formula_blocking_b1(&mut model);
    let style_before = model.get_cell_style(0, 1, 1).unwrap();

    let result = model.set_user_inputs(&[
        (0, 1, 1, OTHER_URL.to_string()), // A1 — succeeds, retargets the link
        (0, 1, 2, "boom".to_string()),    // B1 — inside the array formula, fails
    ]);
    assert!(result.is_err());

    // The pre-batch link is back, not merely absent.
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), URL);
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));
    assert_eq!(model.get_cell_style(0, 1, 1).unwrap(), style_before);
}

#[test]
fn the_link_style_diff_is_recorded_only_when_the_link_is_new() {
    let mut model = new_empty_user_model();

    // A1 is empty: the batch creates a link, which also styles the cell, so the style
    // change must be recorded.
    model
        .set_user_inputs(&[(0, 1, 1, URL.to_string())])
        .unwrap();
    assert_eq!(
        recorded_diffs(&mut model),
        ["SetCellValue", "SetCellStyle", "SetCellLink"]
    );

    // A1 already carries a link, so retargeting it changes no styling and must record
    // no style diff — a spurious one would be an invisible no-op in cell state, but it
    // would still travel to every peer and sit in the history.
    model
        .set_user_inputs(&[(0, 1, 1, OTHER_URL.to_string())])
        .unwrap();
    assert_eq!(recorded_diffs(&mut model), ["SetCellValue", "SetCellLink"]);

    // A plain value creates no link at all, so neither.
    model
        .set_user_inputs(&[(0, 5, 5, "plain".to_string())])
        .unwrap();
    assert_eq!(recorded_diffs(&mut model), ["SetCellValue"]);

    // Both rules again, within a single batch: an empty cell written twice is new on
    // the first write and already linked on the second, so exactly one style diff is
    // recorded. This is what the per-write `old_link` snapshot buys — snapshotting it
    // once up front would leave it `None` for the second write and record a second,
    // spurious style diff.
    model
        .set_user_inputs(&[(0, 9, 9, URL.to_string()), (0, 9, 9, OTHER_URL.to_string())])
        .unwrap();
    assert_eq!(
        recorded_diffs(&mut model),
        [
            "SetCellValue",
            "SetCellStyle",
            "SetCellLink",
            "SetCellValue",
            "SetCellLink"
        ]
    );
}

#[test]
fn a_cell_listed_twice_undoes_through_its_whole_link_chain() {
    let mut model = new_empty_user_model();
    // A1 starts out linked, and the batch retargets it twice. The cell's link diffs
    // therefore form a chain — URL→OTHER_URL, then OTHER_URL→THIRD_URL — that undo has
    // to unwind in reverse to land back on the original target.
    //
    // The pre-existing link is what makes this discriminating. Undo replays the list in
    // reverse, so any arrangement that keeps only the *last* link diff of a cell — the
    // obvious "dedupe the redundant diffs" refactor — stops at the intermediate target
    // instead of the pre-batch one. From an empty cell that would be invisible: the
    // style diff's undo takes the `range_clear_all` branch for a now-empty cell, which
    // drops the link whatever the link diffs said.
    model.set_user_input(0, 1, 1, URL).unwrap();
    model.flush_send_queue(); // drop the seed's diffs so recorded_diffs sees the batch alone

    model
        .set_user_inputs(&[
            (0, 1, 1, OTHER_URL.to_string()),
            (0, 1, 1, THIRD_URL.to_string()),
        ])
        .unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), THIRD_URL);
    assert_eq!(
        model.get_cell_link(0, 1, 1).unwrap(),
        Some(external(THIRD_URL))
    );
    // Read the diffs before undoing (it drains the queue), but assert them after, so
    // that a broken chain fails on the undo — the property this test is named for —
    // rather than on the diff shape.
    let recorded = recorded_diffs(&mut model);

    // One undo walks the chain all the way back to the pre-batch target.
    model.undo().unwrap();
    assert_eq!(model.get_cell_content(0, 1, 1).unwrap(), URL);
    assert_eq!(model.get_cell_link(0, 1, 1).unwrap(), Some(external(URL)));

    // One link diff per write — both retargets, so neither records a style diff.
    assert_eq!(
        recorded,
        ["SetCellValue", "SetCellLink", "SetCellValue", "SetCellLink"]
    );
}

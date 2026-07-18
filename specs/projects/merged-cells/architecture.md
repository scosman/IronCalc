---
status: complete
---

# Architecture: Merged Cells (Core / Model)

Single architecture doc (no separate component designs): the work is confined to
the `base` crate plus thin binding passthroughs, and closely mirrors two existing,
well-tested patterns — conditional formatting (for the diff/undo shape) and array
formulas (for the anchor/guard shape).

All file:line references are against the current tree.

## 1. Data model

### Storage (unchanged)

`Worksheet.merge_cells: Vec<String>` (`base/src/types.rs:194`) stays as-is: a list
of A1 range strings like `"B2:D4"`. Keeping the type preserves the bitcode
`Encode/Decode` layout of `Worksheet` and the xlsx import/export paths
(`xlsx/src/import/worksheets.rs:168`, `xlsx/src/export/worksheets.rs:463`) with
zero migration. This matches the existing convention where conditional formatting
also stores `range: String`.

### Typed view (new)

A normalized, index-based view used by the API and internally:

```rust
// base/src/types.rs (near SheetProperties)
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct MergeCell {
    pub row: i32,
    pub column: i32,
    pub width: i32,   // >= 1
    pub height: i32,  // >= 1
}
```

`MergeCell` is a value type (not stored). It converts to/from the stored A1
string via helpers built on the existing `parse_reference_a1`
(`base/src/expressions/utils/mod.rs:154`), `column_to_number`, and
`number_to_column`. Its shape intentionally matches `Area` (minus `sheet`).

### Worksheet helpers (new, `base/src/worksheet.rs`)

Pure, borrow-only helpers — linear scans over `merge_cells`:

```rust
impl Worksheet {
    // All regions parsed to MergeCell (skips any unparseable string defensively).
    pub(crate) fn merge_cells_parsed(&self) -> Vec<MergeCell>;

    // The region covering (row, column) — anchor or interior — if any.
    pub(crate) fn merge_at(&self, row: i32, column: i32) -> Option<MergeCell>;

    fn merge_contains(m: &MergeCell, row: i32, column: i32) -> bool; // helper
}
```

`merge_at` is the workhorse: used by the edit guard, the clear logic, and the
query API. O(regions) per call is fine given the size constraint.

## 2. Component breakdown

New module `base/src/user_model/merge_cells.rs` (mirrors
`user_model/conditional_formatting.rs` exactly in shape). Wired into
`user_model/mod.rs`.

Touched files and responsibilities:

| File | Change |
|---|---|
| `base/src/types.rs` | add `MergeCell` struct |
| `base/src/worksheet.rs` | add `merge_cells_parsed`, `merge_at`, A1<->index helpers |
| `base/src/user_model/merge_cells.rs` (new) | the 4 public `UserModel` methods |
| `base/src/user_model/history.rs` | add `Diff::MergeCells`, `Diff::UnmergeCells` |
| `base/src/user_model/undo_redo.rs` | handle both diffs in undo + redo matches |
| `base/src/user_model/common.rs` | edit guard in `set_user_input`; unmerge-on-clear in `range_clear_all` |
| `base/src/actions.rs` | `displace_merge_cells` + 6 call sites; move/insert/delete split guards |
| `base/src/user_model/mod.rs` | `mod merge_cells;` |
| `bindings/wasm/src/lib.rs` | 4 passthroughs |
| `bindings/python/…`, `bindings/nodejs/…` | 4 passthroughs each |

## 3. Public interface (base `Model` + `UserModel`)

Base `Model` gets the primitive mutations (no history), `UserModel` wraps them
with diffs — same split as `add_conditional_formatting` /
`Model::add_conditional_formatting`.

```rust
// base Model — primitive, returns data needed to build the Diff
impl Model {
    // Validates, clears covered content, appends range. Returns the covered
    // cells it cleared (row-major, Option per cell) so UserModel can build undo.
    pub fn merge_cells(&mut self, sheet: u32, row: i32, column: i32, width: i32, height: i32)
        -> Result<Vec<Vec<Option<Cell>>>, String>;

    // Removes the region containing (row,column). Returns the removed MergeCell
    // (or None if none). No content change.
    pub fn unmerge_cells(&mut self, sheet: u32, row: i32, column: i32)
        -> Result<Option<MergeCell>, String>;

    pub fn get_merge_cells(&self, sheet: u32) -> Result<Vec<MergeCell>, String>;
    pub fn get_merge_cell(&self, sheet: u32, row: i32, column: i32) -> Result<Option<MergeCell>, String>;
}
```

`UserModel` methods (public API, see functional spec for signatures) call these,
push a diff, and `evaluate_if_not_paused()`.

## 4. Undo/redo

### New `Diff` variants (`base/src/user_model/history.rs:23`)

```rust
MergeCells {
    sheet: u32,
    row: i32,
    column: i32,
    width: i32,
    height: i32,
    // Content discarded from covered cells, row-major over the covered region
    // (excluding the anchor). Used to restore on undo.
    old_covered: Vec<Vec<Option<Cell>>>,
},
UnmergeCells {
    sheet: u32,
    row: i32,
    column: i32,
    width: i32,
    height: i32,
    // Covered cells were empty; nothing to restore. Range is enough to re-add.
},
```

### Handling (`base/src/user_model/undo_redo.rs`)

Both matches are exhaustive over `Diff`, so the compiler forces both handlers.

- **Redo `MergeCells` / Undo `UnmergeCells`** → add the range, clear covered
  content (redo) / (unmerge undo: covered already empty) — factor a shared
  `apply_merge(ws, m)` on the base model.
- **Undo `MergeCells`** → remove the range and restore `old_covered` into the
  covered cells.
- **Redo `UnmergeCells`** → remove the range.

Pattern copied from the CF add/delete pair (`undo_redo.rs:538` undo side,
`:614+` redo side).

## 5. Displacement — the core algorithm (`base/src/actions.rs`)

The single displacement engine has **6 call sites** that today call
`displace_cells(&disp)` + `displace_cf_ranges(sheet, &disp)`:

- `insert_columns` (`:523`), `delete_columns` (`:614`)
- `insert_rows` (`:857`), `delete_rows` (`:926`)
- `move_column_unchecked` (`:1050`), `move_row_unchecked` (`:1164`)

Add a sibling `displace_merge_cells(&mut self, sheet: u32, disp: &DisplaceData)`
and call it at all six, next to the CF call.

`DisplaceData` (`base/src/expressions/parser/stringify.rs:10`) already models the
needed cases: `Row{row,delta}`, `Column{column,delta}`, `RowMove`, `ColumnMove`
(delta may be negative for deletes).

### Per-region transform (row case; column is symmetric)

Let a region span rows `[r1, r2]` (`r2 = row + height - 1`). For an insert/delete
at row `p` with signed `delta` (insert: `delta = +count`; delete:
`delta = -count`, affecting rows `[p, p-delta-1]`):

**Insert (`delta > 0`) at `p`:**
- `p <= r1` → shift: `r1 += delta`, `r2 += delta`.
- `r1 < p <= r2` → grow: `r2 += delta`. (row inserted *inside* the region)
- `p > r2` → unaffected.

**Delete (`delta < 0`, `n = -delta` rows deleted at `[p, p+n-1]`):**
Compute the region's surviving rows after removing the intersection with the
deleted band, then re-derive `[r1', r2']`:
- deleted band entirely below (`p > r2`) → unaffected.
- deleted band entirely above (`p + n <= r1`) → shift up by `n`.
- overlap → new top = `min(r1, p)` after collapse; new height =
  `height - overlap_rows`.
- If resulting `height <= 0` (region fully deleted) **or** the region collapses
  to `1×1` (`new_height == 1 && width == 1`) → **drop the region**.

Column case identical with `c1/c2/width`.

**Move** (`RowMove`/`ColumnMove`, from `move_columns_action` `:1337` /
`move_rows_action` `:1384`): these move a contiguous block by `delta`. A move
**splits** a region when the moved block covers *some but not all* of the
region's rows/columns. Splits are rejected up-front (see §6). A move that
contains the whole region, or is disjoint from it, shifts the region's
coordinates by the same rule as insert/delete of the affected band.

### Implementation shape

Mirror `displace_cf_ranges` (`actions.rs:305`): read the sheet's `merge_cells`,
map each through the transform, drop `None` results, write back the new
`Vec<String>`. No formula/parser involvement (unlike CF), so it's simpler — a
single pass, no two-phase borrow dance.

## 6. Validation & guards

### Merge validation (`Model::merge_cells`)

In order, each `Err(String)` with a clear message, no mutation before all checks
pass:

1. `worksheet(sheet)?` — sheet exists.
2. Bounds: `row >= 1 && column >= 1 && row+height-1 <= LAST_ROW &&
   column+width-1 <= LAST_COLUMN` (`constants.rs:22,25`).
3. Degenerate: `width >= 1 && height >= 1 && !(width == 1 && height == 1)`.
4. Overlap: no existing region intersects the new rect (rect-intersect test over
   `merge_cells_parsed`).
5. Array/spill collision: no cell in the rect is an array-formula anchor or a
   `Cell::SpillCell`. Reuse the same detection the array-split guards use
   (`actions.rs:678-762` helpers / `Cell::SpillCell` match at `model.rs`).

### Move split guard (`move_columns_action` / `move_rows_action`)

Before performing a move, if any region would be split, return
`Err("Cannot move because it would split a merged cell")` — same shape and site
as the existing array-formula guard. Insert/delete never split (they grow/shrink
or drop), so no new guard is needed there beyond the transform; the existing
array guards remain.

### Edit guard (`set_user_input`, `common.rs:398`)

At the top, after the row/column validity checks:

```rust
if let Some(m) = ws.merge_at(row, column) {
    if !(m.row == row && m.column == column) {
        return Err(format!(
            "cannot edit a cell inside a merged region; edit the anchor at {}",
            /* A1 of (m.row, m.column) */
        ));
    }
}
```

### Clear-all unmerge (`range_clear_all`, `common.rs:656`)

After clearing content/formatting for the `Area`, drop any region **fully
contained** in the area and record an `UnmergeCells` diff per dropped region
(bundled into the same diff list so one undo reverts the whole Clear-All).
`range_clear_contents` and `range_clear_formatting` do not touch merges.

## 7. Bindings

Thin passthroughs, one per method, matching existing style:

- **wasm** (`bindings/wasm/src/lib.rs`): `merge_cells`, `unmerge_cells`,
  `get_merge_cells` (returns `JsValue` via `serde_wasm_bindgen::to_value`, like
  `get_worksheets_properties:628`, with `unchecked_return_type = "MergeCell[]"`),
  `get_merge_cell` (returns `MergeCell | undefined`).
- **python / nodejs**: same four, following each binding's existing wrapper
  conventions.

## 8. Error handling

All model methods return `Result<_, String>`, consistent with the crate. No
panics: parsing helpers return `Option` and are treated defensively (an
unparseable stored range is skipped, never unwrapped). Validation is all-or-
nothing: no state is mutated until every check passes, so a rejected `merge_cells`
leaves the workbook untouched and produces no history entry.

## 9. Testing strategy

Rust unit/integration tests under `base/src/test/user_model/` (new
`test_merge_cells.rs`), plus reuse of the existing xlsx round-trip test.

**API + undo/redo**
- merge → `get_merge_cells` reflects it; anchor value kept, covered cleared.
- merge → undo restores covered content and removes region; redo re-applies.
- unmerge → region gone; undo restores it; redo removes again.
- `get_merge_cell` returns the region for anchor and for each covered cell; `None`
  elsewhere.

**Validation (each returns Err, workbook unchanged)**
- out of bounds; 1×1; zero/negative dims.
- overlap with an existing region.
- merge intersecting an array formula / spill.

**Edit + clear guards**
- `set_user_input` on covered cell → Err; on anchor → Ok.
- `range_clear_all` over a region → unmerged (+ undo restores the merge);
  `range_clear_contents` → merge intact.

**Displacement (each with undo/redo)** — one test per case:
- insert rows/cols above → shift; inside → grow.
- delete rows/cols above → shift; overlapping → shrink; consuming region → drop;
  shrink-to-1×1 → drop.
- move whole region → shift; move splitting region → Err.
- symmetric row/column coverage.

**Sheet ops (regression)**
- duplicate sheet copies merges; delete-sheet undo restores merges.

**Round-trip**
- existing `xlsx/tests/test.rs::test_exporting_merged_cells` still passes;
  add a case: import → merge via API → export → re-import equals expected.

Run the project's standard checks (build + clippy + `cargo test`) after each
phase.

## 10. Rejected / deferred alternatives

- **Change `merge_cells` to a typed `Vec<MergeCell>`** — rejected: breaks bitcode
  layout and forces xlsx path rewrites for no functional gain.
- **Coverage index (HashMap covered→anchor)** — deferred: unnecessary at expected
  merge counts; linear scan is simpler and fast enough. Can be added behind the
  same `merge_at` API later without touching callers.
- **Auto-redirect covered-cell writes to the anchor** — rejected: hides merges
  from scripted callers; explicit Err + `get_merge_cell` is cleaner.
- **Clipboard merge fidelity** — deferred to Phase 2 (documented limitation);
  isolated from the core API.

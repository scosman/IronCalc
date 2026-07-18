---
status: complete
---

# Phase 4: Bindings + round-trip

## Overview

The four merged-cell methods (`merge_cells`, `unmerge_cells`, `get_merge_cells`,
`get_merge_cell`) already exist on the base `UserModel` (Phases 1-3). This phase
exposes them to the three language bindings (wasm, python, nodejs) as thin
passthroughs matching each binding's existing conventions, verifies the xlsx
round-trip survives merges created through the new API, and documents the
Phase-2 clipboard limitation.

`MergeCell` (`base/src/types.rs`) already derives `Serialize`/`Deserialize`, so
it crosses each boundary the same way peer value types do (serde-wasm-bindgen
for wasm, `env.to_js_value` for nodejs, a `#[pyclass]` mirror for python).

## Steps

1. **wasm** (`bindings/wasm/src/lib.rs`): add four `#[wasm_bindgen]` methods on
   `Model`, mirroring the conditional-formatting passthroughs.
   - `mergeCells(sheet, row, column, width, height) -> Result<(), JsError>`
   - `unmergeCells(sheet, row, column) -> Result<(), JsError>`
   - `getMergeCells(sheet) -> JsValue` with `unchecked_return_type = "MergeCell[]"`,
     serialized via `serde_wasm_bindgen::to_value`.
   - `getMergeCell(sheet, row, column) -> JsValue` with
     `unchecked_return_type = "MergeCell | null"`, serializing `Option<MergeCell>`.
2. **wasm types** (`bindings/wasm/types.ts`): add the `MergeCell` interface so the
   `unchecked_return_type` references resolve for TS consumers.
3. **nodejs** (`bindings/nodejs/src/user_model.rs`): add the same four methods on
   `UserModel`, using `env.to_js_value` for the two getters (mirrors
   `getWorksheetsProperties` / `getDefinedNameList`). Update the generated
   `index.d.ts` to match.
4. **python types** (`bindings/python/src/types.rs`): add a `PyMergeCell`
   `#[pyclass]` with `row`/`column`/`width`/`height` getters and a
   `From<MergeCell>` impl (mirrors `PySheetProperty`).
5. **python** (`bindings/python/src/lib.rs`): add the four methods on
   `PyUserModel`, returning `Vec<PyMergeCell>` / `Option<PyMergeCell>` for the
   getters.
6. **round-trip test** (`xlsx/tests/test.rs`): build a `UserModel`, create a merge
   via `merge_cells`, export to xlsx, re-import, and assert the merge survives
   both as the stored A1 range and through the typed `get_merge_cells` API.
7. **docs** (`docs/src/features/unsupported-features.md`): add a brief note that
   copy/paste does not carry merge fidelity.

## Tests

- `xlsx/tests/test.rs::test_merge_cells_xlsx_round_trip` — merge via API →
  export → re-import; the stored range and typed API both still report `B2:D3`.
- Existing `xlsx/tests/test.rs::test_exporting_merged_cells` still passes.
- `bindings/nodejs/__test__/index.spec.mjs` — a merge/getMergeCells assertion
  (runs only if the nodejs toolchain is available).

## Toolchain notes

wasm-pack and maturin are not installed in this environment, so the wasm and
python binding test suites cannot be executed here; those crates are verified to
compile via `cargo`. The nodejs suite is attempted via pnpm. All Rust-side
checks (fmt, clippy, `cargo test`, xlsx round-trip) are non-negotiable and run.

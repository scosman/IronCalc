import test from 'ava'

import { UserModel, Model } from '../index.js';
 
test('User Model smoke test', (t) => {
  const model = new UserModel("Workbook1", "en", "UTC", "en");

  model.setUserInput(0, 1, 1, "=1+1");
  t.is(model.getFormattedCellValue(0, 1, 1), '2');
});


test('Raw API smoke test', (t) => {
  const model = new Model("Workbook1", "en", "UTC", "en");

  model.setUserInput(0, 1, 1, "=1+1");
  model.evaluate();
  t.is(model.getFormattedCellValue(0, 1, 1), '2');
});

test('Merged cells API', (t) => {
  const model = new UserModel("Workbook1", "en", "UTC", "en");

  model.setUserInput(0, 2, 2, "anchor");
  // Merge B2:D3 (anchor B2, spanning 3 columns and 2 rows).
  model.mergeCells(0, 2, 2, 3, 2);

  const regions = model.getMergeCells(0);
  t.is(regions.length, 1);
  t.deepEqual(regions[0], { row: 2, column: 2, width: 3, height: 2 });

  // A covered cell resolves to the same region.
  t.deepEqual(model.getMergeCell(0, 3, 4), { row: 2, column: 2, width: 3, height: 2 });
  // A cell outside any region resolves to null.
  t.is(model.getMergeCell(0, 10, 10), null);

  model.unmergeCells(0, 2, 2);
  t.is(model.getMergeCells(0).length, 0);
});


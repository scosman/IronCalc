def test_merge_and_unmerge(um):
    um.set_user_input(0, 2, 2, "anchor")
    # Merge B2:D3 (anchor B2, spanning 3 columns and 2 rows).
    um.merge_cells(0, 2, 2, 3, 2)

    regions = um.get_merge_cells(0)
    assert regions == [{"row": 2, "column": 2, "width": 3, "height": 2}]

    um.unmerge_cells(0, 2, 2)
    assert um.get_merge_cells(0) == []


def test_get_merge_cell_resolves_covered_cells(um):
    um.merge_cells(0, 2, 2, 3, 2)

    region = {"row": 2, "column": 2, "width": 3, "height": 2}
    # The anchor and any covered cell resolve to the same region.
    assert um.get_merge_cell(0, 2, 2) == region
    assert um.get_merge_cell(0, 3, 4) == region
    # A cell outside any region resolves to None.
    assert um.get_merge_cell(0, 10, 10) is None

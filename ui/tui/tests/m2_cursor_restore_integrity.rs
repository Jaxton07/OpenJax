use tui_next::history_cell::{CellRole, HistoryCell};

#[test]
fn history_cell_preserves_identity_for_cursor_related_insertions() {
    let cell = HistoryCell::from_plain(42, CellRole::Assistant, "line");
    assert_eq!(cell.id, 42);
    assert!(cell.committed);
}

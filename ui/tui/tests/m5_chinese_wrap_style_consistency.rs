use tui::history_cell::{CellRole, HistoryCell};

#[test]
fn chinese_history_cell_keeps_lines() {
    let cell = HistoryCell::from_plain(1, CellRole::Assistant, "你好，世界");
    assert_eq!(cell.lines.len(), 1);
}

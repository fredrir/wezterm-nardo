pub mod fx;
pub mod modal;
pub mod theme;
pub mod widgets;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Blank every cell of `area` (clipped to the buffer) and give it `style`.
pub(crate) fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    let area = area.intersection(buf.area);
    for pos in area.positions() {
        buf[pos].reset();
        buf[pos].set_style(style);
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    /// Areas every widget must survive: empty, single cell, and a sliver.
    pub(crate) fn tiny_areas() -> [Rect; 3] {
        [Rect::new(0, 0, 0, 0), Rect::new(0, 0, 1, 1), Rect::new(0, 0, 3, 2)]
    }

    pub(crate) fn buffer(area: Rect) -> Buffer {
        Buffer::empty(area)
    }

    pub(crate) fn row(buf: &Buffer, y: u16) -> String {
        (buf.area.left()..buf.area.right()).map(|x| buf[(x, y)].symbol()).collect()
    }
}

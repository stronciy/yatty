use config::{Rgb, Theme};
use render::{RenderCell, RenderGrid, Rgba};
use term::{Color, Snapshot};

pub fn to_render_grid(snapshot: &Snapshot, theme: &Theme) -> RenderGrid {
    let rows = snapshot.lines.len();
    let cols = snapshot.lines.first().map_or(0, Vec::len);
    let mut cells = Vec::with_capacity(rows * cols);
    for line in &snapshot.lines {
        for cell in line {
            let (mut fg, mut bg) = (
                resolve(cell.fg, theme, theme.foreground),
                resolve(cell.bg, theme, theme.background),
            );
            if cell.inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            cells.push(RenderCell {
                ch: cell.ch,
                fg,
                bg,
                wide_spacer: cell.wide_spacer,
            });
        }
    }
    let cursor = (snapshot.cursor.row < rows && snapshot.cursor.col < cols)
        .then_some((snapshot.cursor.row, snapshot.cursor.col));
    RenderGrid {
        cols,
        rows,
        cells,
        cursor,
        cursor_color: rgba(theme.cursor),
        background: rgba(theme.background),
    }
}

fn resolve(color: Color, theme: &Theme, default: Rgb) -> Rgba {
    match color {
        Color::Default => rgba(default),
        Color::Rgb(r, g, b) => rgba(Rgb(r, g, b)),
        Color::Indexed(index) => rgba(indexed(index, theme)),
    }
}

fn indexed(index: u8, theme: &Theme) -> Rgb {
    match index {
        0..=15 => theme.palette[index as usize],
        16..=231 => {
            let i = index - 16;
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            Rgb(level(i / 36), level((i % 36) / 6), level(i % 6))
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            Rgb(v, v, v)
        }
    }
}

fn rgba(Rgb(r, g, b): Rgb) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

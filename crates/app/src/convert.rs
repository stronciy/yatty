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
            if cell.inverse != cell.selected {
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
        bottom_bar: None,
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

pub fn append_footer(grid: &mut RenderGrid, text: &str, theme: &Theme) {
    let fg = rgba(mix(theme.foreground, theme.background, 0.55));
    let bg = rgba(mix(theme.background, theme.foreground, 0.08));
    let mut chars = text.chars();
    for _ in 0..grid.cols {
        let ch = chars.next().unwrap_or(' ');
        grid.cells.push(RenderCell {
            ch,
            fg,
            bg,
            wide_spacer: false,
        });
    }
    grid.rows += 1;
    grid.bottom_bar = Some(bg);
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb(channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

fn rgba(Rgb(r, g, b): Rgb) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

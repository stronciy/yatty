use render::{RenderCell, RenderGrid, Rgba};
use term::{Color, Snapshot};

const DEFAULT_FG: Rgba = [0.75, 0.77, 0.82, 1.0];
const DEFAULT_BG: Rgba = [0.10, 0.105, 0.12, 1.0];
const CURSOR: Rgba = [0.48, 0.64, 0.97, 1.0];

const ANSI: [[u8; 3]; 16] = [
    [0x15, 0x16, 0x1a],
    [0xf7, 0x76, 0x8e],
    [0x9e, 0xce, 0x6a],
    [0xe0, 0xaf, 0x68],
    [0x7a, 0xa2, 0xf7],
    [0xbb, 0x9a, 0xf7],
    [0x7d, 0xcf, 0xff],
    [0xa9, 0xb1, 0xd6],
    [0x41, 0x48, 0x68],
    [0xf7, 0x76, 0x8e],
    [0x9e, 0xce, 0x6a],
    [0xe0, 0xaf, 0x68],
    [0x7a, 0xa2, 0xf7],
    [0xbb, 0x9a, 0xf7],
    [0x7d, 0xcf, 0xff],
    [0xc0, 0xca, 0xf5],
];

pub fn to_render_grid(snapshot: &Snapshot) -> RenderGrid {
    let rows = snapshot.lines.len();
    let cols = snapshot.lines.first().map_or(0, Vec::len);
    let mut cells = Vec::with_capacity(rows * cols);
    for line in &snapshot.lines {
        for cell in line {
            let (mut fg, mut bg) = (resolve(cell.fg, DEFAULT_FG), resolve(cell.bg, DEFAULT_BG));
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
        cursor_color: CURSOR,
    }
}

fn resolve(color: Color, default: Rgba) -> Rgba {
    match color {
        Color::Default => default,
        Color::Rgb(r, g, b) => rgba([r, g, b]),
        Color::Indexed(index) => rgba(indexed(index)),
    }
}

fn indexed(index: u8) -> [u8; 3] {
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let i = index - 16;
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            [level(i / 36), level((i % 36) / 6), level(i % 6)]
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            [v, v, v]
        }
    }
}

fn rgba([r, g, b]: [u8; 3]) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

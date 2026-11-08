mod atlas;
mod renderer;

pub use renderer::Renderer;

pub type Rgba = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderCell {
    pub ch: char,
    pub fg: Rgba,
    pub bg: Rgba,
    pub wide_spacer: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderGrid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<RenderCell>,
    pub cursor: Option<(usize, usize)>,
    pub cursor_color: Rgba,
}

impl RenderGrid {
    pub fn cell(&self, row: usize, col: usize) -> &RenderCell {
        &self.cells[row * self.cols + col]
    }
}

mod atlas;
mod renderer;

pub use renderer::Renderer;

pub type Rgba = [f32; 4];

#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub family: String,
    pub weight: u16,
    pub size_px: f32,
    pub ligatures: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderCell {
    pub ch: char,
    pub fg: Rgba,
    pub bg: Rgba,
    pub wide_spacer: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderGrid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<RenderCell>,
    pub cursor: Option<(usize, usize)>,
    pub cursor_color: Rgba,
    pub cursor_shape: CursorShape,
    /// 0.0 hides the cursor, 1.0 draws it at full strength.
    pub cursor_alpha: f32,
    pub background: Rgba,
    /// Paints the last row as a full-width bar that extends to the bottom edge of the window.
    pub bottom_bar: Option<Rgba>,
}

/// Space in physical pixels between the window edges and the terminal content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RenderGrid {
    pub fn cell(&self, row: usize, col: usize) -> &RenderCell {
        &self.cells[row * self.cols + col]
    }
}

pub fn monospace_families() -> Vec<String> {
    let font_system = cosmic_text::FontSystem::new();
    let mut names: Vec<String> = font_system
        .db()
        .faces()
        .filter(|face| face.monospaced)
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

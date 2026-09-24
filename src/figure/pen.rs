use super::spec::Rgb;
use crate::text::{Font, Orient};

use std::collections::BTreeSet;
use tiny_skia::Path;

/// How a mark is inked.
#[derive(Debug, Clone, Copy)]
pub enum Style {
    Fill,
    /// Stroked, this wide in canvas units.
    Stroke(f32),
}

/// One path an element drew, in canvas units.
pub struct Mark {
    pub path: Path,
    pub style: Style,
    pub color: Rgb,
    /// A letter or symbol, which the viewer's lit-glyph effect may pick up.
    pub glyph: bool,
}

/// What elements draw with. It records marks rather than pixels, so one pass over the
/// elements serves the layer textures, the flat PNG, the glyph catalogue and the culling
/// extents alike — and an element never has to know about any of them.
pub struct Pen<'a> {
    center: [f32; 2],
    font: &'a Font,
    ink: Rgb,
    line_width: f32,
    marks: Vec<Mark>,
    /// Characters asked for that the font does not have.
    missing: BTreeSet<char>,
}

impl<'a> Pen<'a> {
    pub(super) fn new(center: [f32; 2], font: &'a Font, line_width: f32) -> Self {
        Pen { center, font, ink: Rgb([255; 3]), line_width, marks: Vec::new(), missing: BTreeSet::new() }
    }

    pub(super) fn set_ink(&mut self, ink: Rgb) {
        self.ink = ink;
    }

    pub(super) fn finish(self) -> (Vec<Mark>, BTreeSet<char>) {
        (self.marks, self.missing)
    }

    pub fn center(&self) -> [f32; 2] {
        self.center
    }

    pub fn font(&self) -> &Font {
        self.font
    }

    fn mark(&mut self, path: Option<Path>, style: Style, glyph: bool) {
        if let Some(path) = path {
            self.marks.push(Mark { path, style, color: self.ink, glyph });
        }
    }

    /// Stroke `path`, `width` wide or at the figure's `line_width`.
    pub fn line(&mut self, path: Option<Path>, width: Option<f32>) {
        let w = width.unwrap_or(self.line_width);
        self.mark(path, Style::Stroke(w), false);
    }

    /// Fill `path` as plain ink.
    pub fn fill(&mut self, path: Option<Path>) {
        self.mark(path, Style::Fill, false);
    }

    /// Fill `path` as a letter or symbol.
    pub fn glyph(&mut self, path: Option<Path>) {
        self.mark(path, Style::Fill, true);
    }

    /// Write `s` around the centre, centred on internal angle `alpha`, baseline at `r`.
    pub fn text(&mut self, alpha: f32, r: f32, size: f32, s: &str, orient: Orient) {
        self.note_missing(s);
        for p in self.font.place(self.center, alpha, r, size, s, orient) {
            self.glyph(Some(p));
        }
    }

    /// Write `s` square to the page, centred `(dx, dy)` from the centre.
    pub fn text_at(&mut self, dx: f32, dy: f32, size: f32, s: &str) {
        self.note_missing(s);
        let [cx, cy] = self.center;
        for p in self.font.place_at(cx + dx, cy + dy, size, s) {
            self.glyph(Some(p));
        }
    }

    fn note_missing(&mut self, s: &str) {
        let font = self.font;
        self.missing.extend(s.chars().filter(|&c| !font.has_char(c)));
    }
}

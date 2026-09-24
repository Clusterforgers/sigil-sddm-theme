use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;

use serde::Deserialize;
use tiny_skia::PathBuilder;

/// `{ type: "circle", r: 384 }` — a ring. A `width` makes it a band.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Circle {
    pub r: f32,
    /// Line width; defaults to the figure's `line_width`.
    pub width: Option<f32>,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

impl Draw for Circle {
    fn draw(&self, pen: &mut Pen) {
        let [cx, cy] = pen.center();
        pen.line(PathBuilder::from_circle(cx, cy, self.r), self.width);
    }

    fn check(&self) -> Result<(), String> {
        positive("r", self.r)?;
        self.width.map_or(Ok(()), |w| positive("width", w))
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}

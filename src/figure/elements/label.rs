use super::{positive, Draw};
use crate::figure::pen::Pen;
use crate::figure::spec::Rgb;
use crate::geometric::clockwise;
use crate::text::Orient;

use serde::Deserialize;

/// A single piece of text placed by hand.
///
/// ```json5
/// { type: "label", text: "Z", size: 26, at: { r: 74, angle: 342.9 } } // on the arc
/// { type: "label", text: "VA", size: 13, at: [0, -18] }              // square to the page
/// ```
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub text: String,
    pub size: f32,
    pub at: Place,
    pub color: Option<Rgb>,
    #[serde(rename = "note")]
    _note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Place {
    /// Baseline at radius `r`, centred `angle` degrees clockwise from North, upright to the
    /// centre.
    Arc { r: f32, angle: f32 },
    /// Centred this far from the figure's centre, square to the page.
    Offset([f32; 2]),
}

impl Draw for Label {
    fn draw(&self, pen: &mut Pen) {
        match self.at {
            Place::Arc { r, angle } => pen.text(clockwise(angle), r, self.size, &self.text, Orient::Arc),
            Place::Offset([dx, dy]) => pen.text_at(dx, dy, self.size, &self.text),
        }
    }

    fn check(&self) -> Result<(), String> {
        positive("size", self.size)
    }

    fn color(&self) -> Option<Rgb> {
        self.color
    }
}

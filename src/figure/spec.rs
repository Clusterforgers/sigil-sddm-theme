//! The shape of a figure file. Field docs here are the reference for writing one.

use super::elements::Element;

use serde::Deserialize;
use std::path::PathBuf;

/// The whole figure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The frame the figure is composed in, in canvas units. The viewer fits it to the
    /// window; `imagespin draw` writes a PNG this size (times `--scale`).
    pub canvas: [f32; 2],
    /// Centre of the figure; every radius and angle is measured from here.
    pub center: [f32; 2],
    pub background: Rgb,
    /// Default colour of every element; any element may set its own `color`.
    pub ink: Rgb,
    /// Default width of every stroked line.
    #[serde(default = "default_line_width")]
    pub line_width: f32,
    /// A `.ttf`/`.otf` to letter the figure in, relative to the figure file. Without one,
    /// the embedded face is used.
    #[serde(default)]
    pub font: Option<PathBuf>,
    /// One full loop of the animation; every layer's `turns` are per loop.
    #[serde(rename = "loop")]
    pub timing: Loop,
    /// How often each ambient effect happens; see `effects::Settings`. Optional.
    #[serde(default)]
    pub effects: crate::effects::Settings,
    /// Stacked by each layer's `z`, then by the order written here: a later layer draws
    /// over an earlier one with the same `z`.
    pub layers: Vec<LayerSpec>,
}

fn default_line_width() -> f32 {
    1.6
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    pub frames: u32,
    pub fps: u32,
}

impl Loop {
    pub fn seconds(&self) -> f32 {
        self.frames as f32 / self.fps.max(1) as f32
    }
}

/// A group of elements that turns as one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    pub name: String,
    /// Whole revolutions per loop, positive clockwise. Whole numbers keep the loop seamless.
    pub turns: i32,
    /// Stacking: a higher `z` draws on top. Layers with the same `z` (the default is 0) keep
    /// the order they are written in.
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default, rename = "note")]
    _note: Option<String>,
}

/// A colour, written `"#RRGGBB"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Rgb(pub [u8; 3]);

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        let hex = s.strip_prefix('#').filter(|h| h.len() == 6);
        let v = hex.and_then(|h| u32::from_str_radix(h, 16).ok());
        v.map(|v| Rgb([(v >> 16) as u8, (v >> 8) as u8, v as u8]))
            .ok_or_else(|| format!("expected a colour like \"#FBB929\", got {s:?}"))
    }
}

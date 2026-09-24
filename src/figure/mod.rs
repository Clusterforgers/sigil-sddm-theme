//! The figure: read from a JSON5 file (see `docs/figure.md`), checked, and drawn.
//!
//! ```text
//! figure.json5 ──load──▶ Spec ──check──▶ Figure ──record──▶ marks per layer
//!                                                             ├─▶ render()    one texture per layer, for the viewer
//!                                                             └─▶ composite() one flat PNG
//! ```

mod build;
mod elements;
mod error;
mod pen;
mod render;
mod spec;
mod symbols;
mod watch;

pub use error::FigureError;
pub use render::{Frame, Glyph, LayerImage, Rendered, LAYER_TINT};
pub use spec::{LayerSpec, Loop, Rgb, Spec};
pub use watch::watch;

use crate::text::Font;
use pen::{Mark, Pen};

use image::RgbImage;
use std::collections::HashSet;
use std::path::Path;

/// A figure file, parsed and checked.
pub struct Figure {
    spec: Spec,
    font: Font,
}

impl Figure {
    /// Read and check the figure at `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, FigureError> {
        let path = path.as_ref();
        let src = std::fs::read_to_string(path)
            .map_err(|e| FigureError::new(format!("cannot read: {e}")).file(path))?;
        let base = path.parent().unwrap_or(Path::new("."));
        Self::parse(&src, base).map_err(|e| e.file(path))
    }

    /// Parse and check figure source; a `font` in it is looked up relative to `base`.
    pub fn parse(src: &str, base: &Path) -> Result<Self, FigureError> {
        let mut de = json5::Deserializer::from_str(src);
        let mut spec: Spec = serde_path_to_error::deserialize(&mut de).map_err(|e| FigureError::parse(src, e))?;
        // Checked in file order, so `layers[i]` in a message is the i-th block in the file...
        check(&spec)?;
        // ...then put in stacking order, bottom first, which is the only order anything
        // downstream sees. The sort is stable, so equal `z` keeps file order.
        spec.layers.sort_by_key(|l| l.z);
        let font = match &spec.font {
            Some(f) => Font::from_file(&base.join(f)).map_err(|m| FigureError::new(m).context("font"))?,
            None => Font::embedded(),
        };
        Ok(Figure { spec, font })
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Every layer's marks, bottom first, and a warning for each character the font lacks.
    fn record(&self) -> (Vec<Vec<Mark>>, Vec<String>) {
        let mut warnings = Vec::new();
        let layers = self
            .spec
            .layers
            .iter()
            .map(|layer| {
                let mut pen = Pen::new(self.spec.center, &self.font, self.spec.line_width);
                for e in &layer.elements {
                    pen.set_ink(e.color().unwrap_or(self.spec.ink));
                    e.draw(&mut pen);
                }
                let (marks, missing) = pen.finish();
                if !missing.is_empty() {
                    let chars: String = missing.into_iter().collect();
                    warnings.push(format!("layer {:?}: the font has no glyph for {chars:?}", layer.name));
                }
                marks
            })
            .collect();
        (layers, warnings)
    }

    /// One transparent texture per layer, `scale` pixels per canvas unit.
    pub fn render(&self, scale: f32) -> Rendered {
        let (marks, warnings) = self.record();
        let c = self.spec.center;
        let extents: Vec<[f32; 2]> = marks.iter().map(|m| render::extent(m, c)).collect();
        let disc = extents.iter().map(|e| e[1]).fold(0.0, f32::max);
        let frame = render::frame(c, disc);
        let moments = build::choreograph(&marks, &extents, c, disc);

        let layers = self
            .spec
            .layers
            .iter()
            .zip(&marks)
            .zip(&extents)
            .zip(&moments)
            .map(|(((spec, marks), &extent), when)| {
                let (reveal, reveal_side) = render::reveal_map(marks, when, frame, scale);
                LayerImage {
                    name: spec.name.clone(),
                    turns: spec.turns,
                    extent,
                    image: render::layer_image(marks, frame, scale),
                    reveal,
                    reveal_side,
                }
            })
            .collect();

        let mut warnings = warnings;
        warnings.extend(collisions(&self.spec.layers, &extents));

        Rendered {
            canvas: self.spec.canvas,
            center: c,
            background: self.spec.background,
            frame,
            scale,
            loop_secs: self.spec.timing.seconds(),
            effects: self.spec.effects,
            layers,
            glyphs: marks.iter().enumerate().flat_map(|(i, m)| render::glyphs(m, i)).collect(),
            warnings,
        }
    }

    /// The whole figure flattened onto its background, at `scale` times the canvas. With
    /// `tint`, each layer is painted in its `LAYER_TINT` colour.
    pub fn composite(&self, scale: f32, tint: bool) -> RgbImage {
        let (marks, _) = self.record();
        render::composite(&marks, self.spec.canvas, self.spec.background, scale, tint)
    }
}

/// Pairs of layers that will run into each other: they turn at different rates, and both
/// have ink at some of the same distances from the centre, so sooner or later a line of one
/// sweeps across a line of the other.
///
/// Judged by each layer's radius range, so it can warn about ink that would in fact slip
/// through a gap — but it never misses a real collision.
fn collisions(layers: &[LayerSpec], extents: &[[f32; 2]]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, (a, ea)) in layers.iter().zip(extents).enumerate() {
        for (b, eb) in layers.iter().zip(extents).skip(i + 1) {
            let (lo, hi) = (ea[0].max(eb[0]), ea[1].min(eb[1]));
            let empty = |e: &[f32; 2]| e[1] <= 0.0;
            if a.turns != b.turns && lo < hi && !empty(ea) && !empty(eb) {
                out.push(format!(
                    "layers {:?} (turns {:+}) and {:?} (turns {:+}) both have ink between r = {lo:.1} \
                     and {hi:.1}, and turn at different rates: their lines will cross",
                    a.name, a.turns, b.name, b.turns
                ));
            }
        }
    }
    out
}

/// Everything a figure must satisfy beyond parsing.
fn check(spec: &Spec) -> Result<(), FigureError> {
    let bad = |what: &str, msg: String| Err(FigureError::new(msg).context(what));

    if spec.canvas.iter().any(|&v| v <= 0.0) {
        return bad("canvas", "both sides must be greater than 0".into());
    }
    if spec.line_width <= 0.0 {
        return bad("line_width", "must be greater than 0".into());
    }
    if spec.timing.frames == 0 || spec.timing.fps == 0 {
        return bad("loop", "`frames` and `fps` must both be at least 1".into());
    }
    if let Err(msg) = spec.effects.check() {
        return bad("effects", msg);
    }
    let max = crate::gpu::MAX_LAYERS;
    if spec.layers.is_empty() || spec.layers.len() > max {
        return bad("layers", format!("need between 1 and {max} layers, got {}", spec.layers.len()));
    }

    let mut names = HashSet::new();
    for (i, layer) in spec.layers.iter().enumerate() {
        let here = format!("layers[{i}] {:?}", layer.name);
        if !names.insert(layer.name.as_str()) {
            return bad(&here, "another layer already has this name".into());
        }
        for (j, e) in layer.elements.iter().enumerate() {
            if let Err(msg) = e.check() {
                return bad(&format!("{here} › elements[{j}] ({})", e.kind()), msg);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r##"{
        canvas: [400, 400], center: [200, 200],
        background: "#000000", ink: "#FFFFFF",
        loop: { frames: 10, fps: 10 },
        layers: [
            { name: "a", turns: 1, elements: [ { type: "circle", r: 50 } ] },
            { name: "b", turns: -1, elements: [ { type: "text", along: { circle: 100 }, size: 12, items: ["AB", "{cross}"] } ] },
        ],
    }"##;

    fn parse(src: &str) -> Result<Figure, FigureError> {
        Figure::parse(src, Path::new("."))
    }

    fn error(src: &str) -> String {
        parse(src).err().expect("should not load").to_string()
    }

    #[test]
    fn the_shipped_figure_loads_and_draws() {
        let fig = Figure::load(concat!(env!("CARGO_MANIFEST_DIR"), "/figure.json5")).unwrap_or_else(|e| panic!("{e}"));
        let r = fig.render(0.5);
        assert_eq!(r.layers.len(), fig.spec().layers.len());
        assert!(r.glyphs.len() > 300, "only {} glyphs", r.glyphs.len());
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn glyphs_know_their_layer() {
        let r = parse(MINIMAL).unwrap().render(1.0);
        // "A", "B" and the cross, all in layer 1; the circle is not a glyph.
        assert_eq!(r.glyphs.len(), 3);
        assert!(r.glyphs.iter().all(|g| g.layer == 1));
        let e = r.layers[0].extent;
        assert!((e[0] - 49.2).abs() < 0.1 && (e[1] - 50.8).abs() < 0.1, "{e:?}");
    }

    #[test]
    fn a_misspelled_field_names_the_element_and_line() {
        let e = error(&MINIMAL.replace("r: 50", "radius: 50"));
        assert!(e.contains("layers[0].elements[0]"), "{e}");
        assert!(e.contains("unknown field `radius`"), "{e}");
        // Line and column of the misspelled key itself.
        assert!(e.starts_with("6:66:"), "no line number: {e}");
    }

    #[test]
    fn an_unknown_type_lists_the_known_ones() {
        let e = error(&MINIMAL.replace("\"circle\"", "\"cirle\""));
        assert!(e.contains("unknown variant `cirle`") && e.contains("polygon"), "{e}");
    }

    #[test]
    fn nonsense_values_are_rejected_with_context() {
        let e = error(&MINIMAL.replace("{ type: \"circle\", r: 50 }", "{ type: \"polygon\", sides: 2, r: 50 }"));
        assert!(e.contains(r#"layers[0] "a" › elements[0] (polygon)"#) && e.contains("3 `sides`"), "{e}");
        let e = error(&MINIMAL.replace("{cross}", "{crosss}"));
        assert!(e.contains("unknown variant `crosss`"), "{e}");
    }

    #[test]
    fn z_decides_the_stacking_and_ties_keep_file_order() {
        let src = MINIMAL.replace("name: \"a\", turns: 1,", "name: \"a\", turns: 1, z: 5,");
        let fig = parse(&src).unwrap();
        let names: Vec<_> = fig.spec().layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["b", "a"], "higher z should be drawn last, on top");
        assert_eq!(fig.render(1.0).glyphs[0].layer, 0, "b's glyphs belong to the bottom layer now");

        let fig = parse(MINIMAL).unwrap();
        let names: Vec<_> = fig.spec().layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn layers_that_would_collide_are_warned_about() {
        // Apart: a ring at 50 and text at 100.
        assert!(parse(MINIMAL).unwrap().render(1.0).warnings.is_empty());
        // Both at 100, turning opposite ways.
        let r = parse(&MINIMAL.replace("r: 50", "r: 100")).unwrap().render(1.0);
        assert!(r.warnings.iter().any(|w| w.contains("their lines will cross")), "{:?}", r.warnings);
        // Both at 100, but turning together: nothing ever moves past anything.
        let same = MINIMAL.replace("r: 50", "r: 100").replace("turns: -1", "turns: 1");
        assert!(parse(&same).unwrap().render(1.0).warnings.is_empty());
    }

    #[test]
    fn layer_names_are_unique() {
        let e = error(&MINIMAL.replace("name: \"b\"", "name: \"a\""));
        assert!(e.contains("another layer already has this name"), "{e}");
    }

    #[test]
    fn colours_must_be_hex() {
        let e = error(&MINIMAL.replace("\"#FFFFFF\"", "\"white\""));
        assert!(e.contains("ink") && e.contains("#FBB929"), "{e}");
    }
}

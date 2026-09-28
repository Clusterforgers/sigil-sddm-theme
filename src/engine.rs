//! The figure and its effects, ready to draw, with no window attached. Whatever hosts it —
//! the `live` viewer, the login screen — hands it time, the size it is drawn at and what
//! the person does, and gets back the uniform block for the shader.

use crate::effects::Effects;
use crate::figure::Rendered;
use crate::gpu::{self, Uniforms};

/// Which mip of the artwork to read, given how the window maps onto it.
///
/// One sub-sample covers `1 / (fit * ss)` source texels; when that exceeds one the disc is
/// being minified and level 0 is undersampled, which is what makes the thin gold lines
/// crawl. Below one there is nothing to gain, so the level floors at zero.
///
/// The bias is not a fudge. Trilinear filtering blends toward a full 2x2 box, which is a
/// wider filter than the footprint actually calls for, so the straight `-log2` over-blurs.
/// Fitted against a brute-force `ss=8` render: -0.35 beats both level 0 and an unbiased
/// level on closeness to that reference *and* on high-frequency energy, at 714x427 and at
/// 500x300 alike.
fn source_lod(fit_scale: f32, ss: u32, canvas_scale: f32) -> f32 {
    // `canvas_scale` is how much denser the texture is than the coordinate space, which
    // shifts the whole thing by one level per doubling.
    let rate = (fit_scale * ss as f32 / canvas_scale.max(1e-3)).max(1e-3);
    (-rate.log2() - 0.35).max(0.0)
}

pub struct Engine {
    /// The figure on screen.
    pub fig: Rendered,
    pub fx: Effects,
    uni: Uniforms,
    /// Supersampling factor, 1-4.
    pub ss: u32,
    /// Paint each layer in its own colour, for telling which ring turns with which.
    pub tint: bool,
}

impl Engine {
    pub fn new(fig: Rendered, ss: u32) -> Self {
        let uni = gpu::uniforms(&fig, ss);
        let fx = Effects::new(&fig);
        Engine { fig, fx, uni, ss, tint: false }
    }

    /// Swap in a newly saved figure, keeping the effects in flight and the settings.
    pub fn load(&mut self, fig: Rendered) {
        self.uni = gpu::uniforms(&fig, self.ss);
        self.fx.reload(&fig);
        self.fig = fig;
    }

    /// The uniform block as it stands, for building a pipeline around.
    pub fn uniforms(&self) -> &Uniforms {
        &self.uni
    }

    /// Advance by `dt` seconds and lay out a frame `w` by `h` pixels.
    pub fn frame(&mut self, dt: f32, w: f32, h: f32) -> &Uniforms {
        self.frame_in(dt, [0.0, 0.0, w, h])
    }

    /// The same, with the figure fitted into `area` (x, y, width, height, in pixels) of the
    /// frame rather than all of it. The background and the explosion's flash still cover
    /// the whole frame, which is what a login screen with room under the figure wants.
    pub fn frame_in(&mut self, dt: f32, area: [f32; 4]) -> &Uniforms {
        self.fx.advance(dt);

        // Fit the canvas into the area, preserving aspect.
        let [ax, ay, w, h] = area;
        let [sw, sh] = self.fig.canvas;
        let scale = (w / sw).min(h / sh);
        let n = self.fig.layers.len();
        // The surge shakes the whole picture, which is just moving where it is fitted.
        let [jx, jy] = self.fx.shake();
        let (ox, oy) = (ax + (w - sw * scale) * 0.5 + jx * scale, ay + (h - sh * scale) * 0.5 + jy * scale);
        self.uni.fit = [scale, ox, oy, n as f32];
        self.uni.quality[0] = self.ss as f32;
        self.uni.quality[3] = source_lod(scale, self.ss, self.fig.scale);
        self.uni.look[1] = if self.tint { 1.0 } else { 0.0 };
        self.fx.write(&mut self.uni);
        &self.uni
    }

    /// The pointer at pixel `(x, y)` of the last frame, or gone.
    pub fn pointer(&mut self, at: Option<[f32; 2]>) {
        let [scale, ox, oy, _] = self.uni.fit;
        self.fx.pointer(at.map(|[x, y]| [(x - ox) / scale, (y - oy) / scale]));
    }
}

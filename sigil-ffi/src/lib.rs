//! The engine behind a C interface, for the Qt plugin that draws the sigil on the login
//! screen. `include/sigil.h` declares the same calls; keep the two in step.
//!
//! Every call takes the engine made by `sigil_engine_new` and is safe to make from any one
//! thread at a time. Byte views it hands back stay valid until the next call that changes
//! the engine; the plugin copies them straight away.

use imagespin::engine::Engine;
use imagespin::figure::{Figure, LayerImage};
use imagespin::gpu::{glow_pyramid, source_pyramid};

use std::ffi::{c_char, CStr, CString};
use std::ptr;

pub struct SigilEngine {
    engine: Engine,
    /// Each layer's artwork and glow with their mips, made with the engine so the thread
    /// that makes it pays for them, and dropped once they are on the GPU.
    pyramids: Vec<Option<Pyramids>>,
    /// The last frame's uniform block, kept alive for the caller to copy.
    uniforms: Vec<u8>,
}

struct Pyramids {
    art: (Vec<u8>, u32),
    glow: (Vec<u8>, u32),
}

impl Pyramids {
    fn of(layer: &LayerImage) -> Self {
        Pyramids { art: source_pyramid(&layer.image), glow: glow_pyramid(&layer.image) }
    }
}

impl SigilEngine {
    fn pyramids(&mut self, i: usize) -> Option<&Pyramids> {
        let layer = self.engine.fig.layers.get(i)?;
        Some(self.pyramids[i].get_or_insert_with(|| Pyramids::of(layer)))
    }
}

/// A view of bytes owned by the engine, and how many mip levels they hold.
#[repr(C)]
pub struct SigilBytes {
    pub data: *const u8,
    pub len: usize,
    pub levels: u32,
}

impl SigilBytes {
    fn of(v: &[u8], levels: u32) -> Self {
        SigilBytes { data: v.as_ptr(), len: v.len(), levels }
    }
}

/// Load the figure at `path`, with the JSON settings in `overrides` laid over it if that is
/// not null and the file exists, and draw its layers at `canvas_scale` texels per canvas
/// unit, supersampled `ss` times. Null on failure, with the reason in `*error` if `error` is not
/// null; free that with `sigil_string_free`.
///
/// # Safety
/// `path` is a NUL-terminated string, `overrides` null or one; `error` is null or points at
/// writable storage.
#[no_mangle]
pub unsafe extern "C" fn sigil_engine_new(
    path: *const c_char,
    overrides: *const c_char,
    canvas_scale: f32,
    ss: u32,
    error: *mut *mut c_char,
) -> *mut SigilEngine {
    let fail = |msg: String| {
        if !error.is_null() {
            *error = CString::new(msg).unwrap_or_default().into_raw();
        }
        ptr::null_mut()
    };
    if path.is_null() {
        return fail("no figure path".into());
    }
    let path = CStr::from_ptr(path).to_string_lossy().into_owned();
    let loaded = if overrides.is_null() {
        Figure::load(&path)
    } else {
        Figure::load_with(&path, CStr::from_ptr(overrides).to_string_lossy().as_ref())
    };
    let fig = match loaded {
        Ok(f) => f.render(canvas_scale),
        Err(e) => return fail(e.to_string()),
    };
    // A layer at a time, each on its own thread: this is most of the work of loading.
    let pyramids = std::thread::scope(|s| {
        let jobs: Vec<_> = fig.layers.iter().map(|l| s.spawn(|| Pyramids::of(l))).collect();
        jobs.into_iter().map(|j| j.join().ok()).collect()
    });
    let engine = Engine::new(fig, ss.clamp(1, 4));
    Box::into_raw(Box::new(SigilEngine { engine, pyramids, uniforms: Vec::new() }))
}

/// # Safety
/// `e` came from `sigil_engine_new` and is not used again.
#[no_mangle]
pub unsafe extern "C" fn sigil_engine_free(e: *mut SigilEngine) {
    if !e.is_null() {
        drop(Box::from_raw(e));
    }
}

/// # Safety
/// `s` came from this library and is not used again.
#[no_mangle]
pub unsafe extern "C" fn sigil_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// How many layers there are: the depth of each texture array.
#[no_mangle]
pub extern "C" fn sigil_layer_count(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.len() as u32
}

/// The side of each layer's artwork and glow, in texels, at mip level 0.
#[no_mangle]
pub extern "C" fn sigil_layer_side(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.first().map_or(0, |l| l.image.width())
}

/// The side of each layer's reveal map, in texels. It has no mips.
#[no_mangle]
pub extern "C" fn sigil_reveal_side(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.first().map_or(0, |l| l.reveal_side)
}

/// Layer `i`'s artwork and its mips, smallest last: sRGB-encoded premultiplied RGBA8.
#[no_mangle]
pub extern "C" fn sigil_layer_art(e: &mut SigilEngine, i: u32) -> SigilBytes {
    e.pyramids(i as usize).map_or(SigilBytes::of(&[], 0), |p| SigilBytes::of(&p.art.0, p.art.1))
}

/// Layer `i`'s bloom highlight and its mips, smallest last: linear R8.
#[no_mangle]
pub extern "C" fn sigil_layer_glow(e: &mut SigilEngine, i: u32) -> SigilBytes {
    e.pyramids(i as usize).map_or(SigilBytes::of(&[], 0), |p| SigilBytes::of(&p.glow.0, p.glow.1))
}

/// Layer `i`'s reveal map: RG8, one level.
#[no_mangle]
pub extern "C" fn sigil_layer_reveal(e: &mut SigilEngine, i: u32) -> SigilBytes {
    e.engine.fig.layers.get(i as usize).map_or(SigilBytes::of(&[], 0), |l| SigilBytes::of(&l.reveal, 1))
}

/// The textures are on the GPU: let go of the CPU's copies of the pyramids, a hundred-odd
/// megabytes. Asking for a layer again makes it afresh.
#[no_mangle]
pub extern "C" fn sigil_release_pyramids(e: &mut SigilEngine) {
    e.pyramids.iter_mut().for_each(|p| *p = None);
}

/// The size of the uniform block, in bytes.
#[no_mangle]
pub extern "C" fn sigil_uniforms_size() -> usize {
    std::mem::size_of::<imagespin::gpu::Uniforms>()
}

/// Advance by `dt` seconds and lay out a frame with the figure fitted into the rectangle
/// `(x, y, w, h)`, in the frame's pixels. `flip_height` is the frame's height if the
/// framebuffer runs y-up, else 0; `encode_srgb` is nonzero if the target does not encode
/// sRGB itself. The bytes are the uniform block for the frame.
#[no_mangle]
pub extern "C" fn sigil_frame(
    e: &mut SigilEngine,
    dt: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    flip_height: f32,
    encode_srgb: i32,
) -> SigilBytes {
    let uni = e.engine.frame_in(dt, [x, y, w, h]);
    let mut uni = *uni;
    uni.host = [flip_height, if encode_srgb != 0 { 1.0 } else { 0.0 }];
    e.uniforms.clear();
    e.uniforms.extend_from_slice(bytemuck::bytes_of(&uni));
    SigilBytes::of(&e.uniforms, 1)
}

/// A character was typed.
#[no_mangle]
pub extern "C" fn sigil_key(e: &mut SigilEngine) {
    e.engine.fx.key();
}

/// Backspace.
#[no_mangle]
pub extern "C" fn sigil_backspace(e: &mut SigilEngine) {
    e.engine.fx.backspace();
}

/// Enter: start the surge. Nonzero if it started, zero if one was already running.
#[no_mangle]
pub extern "C" fn sigil_surge(e: &mut SigilEngine) -> i32 {
    e.engine.fx.surge() as i32
}

/// The password was refused.
#[no_mangle]
pub extern "C" fn sigil_fail(e: &mut SigilEngine) {
    e.engine.fx.fail();
}

/// Nonzero once for each explosion of the surge.
#[no_mangle]
pub extern "C" fn sigil_take_detonated(e: &mut SigilEngine) -> i32 {
    e.engine.fx.take_detonated() as i32
}

/// The pointer is at pixel `(x, y)` of the last frame, or has left if `present` is zero.
#[no_mangle]
pub extern "C" fn sigil_pointer(e: &mut SigilEngine, present: i32, x: f32, y: f32) {
    e.engine.pointer((present != 0).then_some([x, y]));
}

/// Nonzero if the figure file asks for the system panel on the login screen.
#[no_mangle]
pub extern "C" fn sigil_system_info(e: &SigilEngine) -> i32 {
    e.engine.fig.login.system_info as i32
}

/// Nonzero if the figure file asks for the debug readout.
#[no_mangle]
pub extern "C" fn sigil_login_debug(e: &SigilEngine) -> i32 {
    e.engine.fig.login.debug as i32
}

/// Samples per pixel side the figure file asks for, or 0 to pick for the screen.
#[no_mangle]
pub extern "C" fn sigil_login_supersample(e: &SigilEngine) -> u32 {
    e.engine.fig.login.supersample.min(4)
}

/// Draw with `ss` samples per pixel side from the next frame on, 1 to 4.
#[no_mangle]
pub extern "C" fn sigil_set_supersample(e: &mut SigilEngine, ss: u32) {
    e.engine.ss = ss.clamp(1, 4);
}

/// Seconds of the explosion the login screen shows before checking the password.
#[no_mangle]
pub extern "C" fn sigil_check_after(e: &SigilEngine) -> f32 {
    e.engine.fig.login.check_after.max(0.0)
}

/// Show the figure whole at once rather than drawing it in.
#[no_mangle]
pub extern "C" fn sigil_skip_build(e: &mut SigilEngine) {
    e.engine.fx.skip_build();
}

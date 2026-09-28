pub const SOURCE: &str = concat!(
    include_str!("../../shaders/common.wgsl"),
    "\n",
    include_str!("../../shaders/effects/bloom.wgsl"),
    "\n",
    include_str!("../../shaders/effects/pulse.wgsl"),
    "\n",
    include_str!("../../shaders/effects/flare.wgsl"),
    "\n",
    include_str!("../../shaders/effects/bolt.wgsl"),
    "\n",
    include_str!("../../shaders/effects/glyph.wgsl"),
    "\n",
    include_str!("../../shaders/effects/ambient.wgsl"),
    "\n",
    include_str!("../../shaders/effects/dissolve.wgsl"),
    "\n",
    include_str!("../../shaders/effects/tint.wgsl"),
    "\n",
    include_str!("../../shaders/effects/scramble.wgsl"),
    "\n",
    include_str!("../../shaders/effects/build.wgsl"),
    "\n",
    include_str!("../../shaders/effects/threads.wgsl"),
    "\n",
    include_str!("../../shaders/effects/interact.wgsl"),
    "\n",
    include_str!("../../shaders/spin.wgsl"),
);

/// The fragment shader as Vulkan-flavoured GLSL 440, which is what Qt's `qsb` takes in and
/// turns into every backend Qt Quick might be running on.
///
/// Texture and sampler come out combined, as GLSL has them, each at its texture's binding:
/// the uniform block at 0, the artwork at 1, the glow at 3 and the reveal map at 4. The
/// sampler's own binding, 2, disappears into them.
pub fn fragment_glsl() -> Result<String, String> {
    use naga::back::glsl;

    let module = naga::front::wgsl::parse_str(SOURCE).map_err(|e| e.emit_to_string(SOURCE))?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
        .validate(&module)
        .map_err(|e| format!("{e:?}"))?;

    let binding = |b: u32| naga::ResourceBinding { group: 0, binding: b };
    let options = glsl::Options {
        version: glsl::Version::Desktop(440),
        // Nothing to adjust: the vertex stage is Qt's, and y is the host's business.
        writer_flags: glsl::WriterFlags::empty(),
        binding_map: [0, 1, 3, 4].into_iter().map(|b| (binding(b), b as u8)).collect(),
        zero_initialize_workgroup_memory: false,
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: naga::ShaderStage::Fragment,
        entry_point: "fs".into(),
        multiview: None,
    };
    let mut out = String::new();
    let mut w = glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, Default::default())
        .map_err(|e| e.to_string())?;
    w.write().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// The Qt plugin's shader comes from this translation, so an edit to the WGSL that it
    /// cannot follow must fail here rather than in a login screen.
    #[test]
    fn the_fragment_shader_translates_for_qt_with_the_bindings_it_expects() {
        let glsl = super::fragment_glsl().unwrap();
        assert!(glsl.starts_with("#version 440"));
        for line in [
            "layout(std140, binding = 0) uniform",
            "layout(binding = 1) uniform sampler2DArray",
            "layout(binding = 3) uniform sampler2DArray",
            "layout(binding = 4) uniform sampler2DArray",
        ] {
            assert!(glsl.contains(line), "missing `{line}`");
        }
    }
}

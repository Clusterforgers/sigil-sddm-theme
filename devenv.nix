{ pkgs, lib, ... }:
let
  # cargo-built binaries get no RPATH, and winit/wgpu dlopen these at runtime
  # rather than linking them, so they must be on the loader path.
  runtimeLibs = with pkgs; [
    # winit (wayland + X11 fallback)
    wayland
    libxkbcommon
    libX11
    libXcursor
    libXrandr
    libXi

    # wgpu
    vulkan-loader
    libGL
  ];
in {
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = with pkgs; [
    pkg-config
    vulkan-tools   # vulkaninfo, for debugging adapter selection
    imagemagick    # `imagespin check` / preview inspection
    ffmpeg         # optional mp4/webm export from rendered frames
  ];

  env.LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;

  enterShell = ''
    echo "$(rustc --version) / $(cargo --version)"
  '';
}

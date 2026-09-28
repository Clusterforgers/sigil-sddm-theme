# The Sigil SDDM theme: the QML theme, and the Qt plugin that draws the live figure with
# the Rust engine. Build it with the same nixpkgs as the system, so the plugin matches the
# Qt that SDDM's greeter runs.
{
  lib,
  stdenv,
  rustPlatform,
  cargo,
  rustc,
  cmake,
  ninja,
  corrosion,
  qt6,
}:

stdenv.mkDerivation {
  pname = "sigil-sddm-theme";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../shaders
      ../fonts
      ../figure.json5
      ../sigil-ffi
      ../qml-plugin
      ../theme
    ];
  };

  # Crates vendored ahead of time: the build has no network.
  cargoDeps = rustPlatform.importCargoLock { lockFile = ../Cargo.lock; };

  nativeBuildInputs = [
    cmake
    ninja
    corrosion
    cargo
    rustc
    rustPlatform.cargoSetupHook
    qt6.qtshadertools
    qt6.wrapQtAppsHook
  ];
  buildInputs = [
    qt6.qtbase
    qt6.qtdeclarative
  ];

  # A QML plugin, not an app: nothing to wrap.
  dontWrapQtApps = true;

  cmakeDir = "../qml-plugin";
  cmakeFlags = [ "-DSIGIL_QML_DIR=lib/qt-6/qml/Sigil" ];

  # The shader the plugin compiles is translated from the WGSL, so it is always the one
  # the engine was built with.
  preConfigure = ''
    cargo run --release --offline --bin export-shaders -- qml-plugin/shaders/sigil.frag
  '';

  postInstall = ''
    theme=$out/share/sddm/themes/sigil
    mkdir -p $theme
    cp -r ../theme/. $theme/
    # On the login screen the figure stays broken after a surge: a password that works
    # takes you away, and one that does not calls the surge off and brings it back.
    sed 's/stay_broken: false/stay_broken: true/' ../figure.json5 > $theme/figure.json5
  '';

  meta = {
    description = "SDDM theme: the Sigillum Dei Aemeth, turning, with the password written in symbols beneath it";
    platforms = lib.platforms.linux;
  };
}

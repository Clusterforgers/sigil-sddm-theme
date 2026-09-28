# The Sigil SDDM theme: the QML theme, and the Qt plugin that draws the live figure with
# the Rust engine. Build it with the same nixpkgs as the system, so the plugin matches the
# Qt that SDDM's greeter runs.
#
# Two derivations: the plugin, which takes minutes to build and changes only with the code,
# and the theme around it, which is copied files and changes with `settings`. So changing a
# setting in the NixOS configuration does not rebuild the engine.
{
  lib,
  stdenv,
  runCommand,
  writeText,
  rustPlatform,
  cargo,
  rustc,
  cmake,
  ninja,
  corrosion,
  qt6,
  # Laid over the theme's figure.json5 when the login screen loads: any setting in that
  # file, e.g. `{ login.system_info = false; effects.lightning.enabled = false; }`.
  settings ? { },
}:

let
  version = "0.1.0";

  plugin = stdenv.mkDerivation {
    pname = "sigil-sddm-plugin";
    inherit version;

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
  };

  # The theme's own default is to stay broken after a surge: a password that works takes you
  # away, and one that does not calls the surge off and brings the figure back.
  overrides = writeText "figure.overrides.json" (
    builtins.toJSON (lib.recursiveUpdate { effects.surge.stay_broken = true; } settings)
  );
in
runCommand "sigil-sddm-theme-${version}"
  {
    passthru = { inherit plugin overrides; };
    meta = {
      description = "SDDM theme: the Sigillum Dei Aemeth, turning, with the password written in symbols beneath it";
      platforms = lib.platforms.linux;
    };
  }
  ''
    theme=$out/share/sddm/themes/sigil
    mkdir -p $theme $out/lib
    cp -r ${../theme}/. $theme/
    cp ${../figure.json5} $theme/figure.json5
    cp ${overrides} $theme/figure.overrides.json
    # The plugin, where SDDM's extraPackages puts it on the greeter's QML import path.
    ln -s ${plugin}/lib/qt-6 $out/lib/qt-6
  ''

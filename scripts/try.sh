#!/usr/bin/env bash
# Build the plugin and stage the theme as it will be installed, then show it:
#
#   scripts/try.sh            the mock preview: password `sigil`, anything else is refused
#   scripts/try.sh demo       the same, typing a wrong password and then the right one itself
#   scripts/try.sh greeter    SDDM's own greeter in test mode (login does nothing there)
#
# Everything comes from the system's nixpkgs, the same Qt that SDDM runs. The plugin builds
# incrementally in build-qt/. Qt logs to the journal unless told otherwise, so it is told.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
build=$root/build-qt
stage=$build/stage
mode=${1:-preview}

# The package's build environment, plus the Qt paths the preview needs, as variables.
shell="let pkgs = (builtins.getFlake \"nixpkgs\").legacyPackages.\${builtins.currentSystem};
in (pkgs.callPackage $root/nix/package.nix {}).plugin.overrideAttrs (_: {
  QTBASE = pkgs.qt6.qtbase; QTDECL = pkgs.qt6.qtdeclarative; QTWAYLAND = pkgs.qt6.qtwayland;
})"

cd "$root"
cargo run -q --release --bin export-shaders -- qml-plugin/shaders/sigil.frag >/dev/null

# The theme as installed: its files, the figure, and the overrides the package writes.
rm -rf "$stage" && mkdir -p "$stage/preview"
cp -r theme "$stage/theme"
cp figure.json5 "$stage/theme/figure.json5"
echo '{"effects":{"surge":{"stay_broken":true}}}' > "$stage/theme/figure.overrides.json"
cp preview/Preview.qml "$stage/preview/"

export QT_FORCE_STDERR_LOGGING=1
export QT_LOGGING_RULES=${QT_LOGGING_RULES:-"sigil.info=true"}

nix-shell -E "$shell" --run "
    set -e
    [ -f '$build/build.ninja' ] || cmake -S qml-plugin -B '$build' -G Ninja -DCMAKE_BUILD_TYPE=Release -DQT_NO_PRIVATE_MODULE_WARNING=ON >/dev/null
    cmake --build '$build' | grep -v '^\[' || true
    if [ '$mode' = greeter ]; then
        QML_IMPORT_PATH='$build' exec sddm-greeter-qt6 --test-mode --theme '$stage/theme'
    fi
    export QT_PLUGIN_PATH=\$QTWAYLAND/lib/qt-6/plugins:\$QTDECL/lib/qt-6/plugins:\$QTBASE/lib/qt-6/plugins
    export QML_IMPORT_PATH='$build':\$QTDECL/lib/qt-6/qml:\$QTWAYLAND/lib/qt-6/qml
    exec \$QTDECL/bin/qml '$stage/preview/Preview.qml' -- $mode
"

cmake_minimum_required(VERSION 3.22)
project(sigil-qml LANGUAGES CXX)

# QQuickRhiItem is 6.7; the plugin must be built against the same Qt that SDDM runs, which
# the Nix package sees to. That is also why the private-module warning is not news.
set(QT_NO_PRIVATE_MODULE_WARNING ON)
find_package(Qt6 6.7 REQUIRED COMPONENTS Core Gui GuiPrivate Quick ShaderTools)
find_package(Corrosion REQUIRED)

qt_standard_project_setup(REQUIRES 6.7)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# The engine, as a static library from the sigil-ffi crate.
corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/../sigil-ffi/Cargo.toml CRATES sigil-ffi)

# One library that is both the module and its plugin: a single .so for the greeter to
# load, with nothing beside it to be found.
qt_add_qml_module(sigil
    URI Sigil
    VERSION 1.0
    PLUGIN_TARGET sigil
    OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/Sigil
    SOURCES SigilItem.h SigilItem.cpp SystemInfo.h SystemInfo.cpp
)

# The fragment shader is generated from the WGSL by `cargo run --bin export-shaders`.
qt_add_shaders(sigil "sigil_shaders"
    PREFIX "/qt/qml/Sigil"
    GLSL "300es,330"
    HLSL 50
    MSL 12
    FILES
        shaders/sigil.vert
        shaders/sigil.frag
)

target_include_directories(sigil PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../sigil-ffi/include)
# QRhi is semi-public: its header comes with the private include paths.
target_link_libraries(sigil PRIVATE Qt6::Quick Qt6::GuiPrivate sigil_ffi)

include(GNUInstallDirs)
set(SIGIL_QML_DIR "${CMAKE_INSTALL_LIBDIR}/qt-6/qml/Sigil" CACHE STRING "Where the Sigil QML module is installed")
install(TARGETS sigil
    LIBRARY DESTINATION ${SIGIL_QML_DIR}
)
install(FILES
    ${CMAKE_CURRENT_BINARY_DIR}/Sigil/qmldir
    ${CMAKE_CURRENT_BINARY_DIR}/Sigil/sigil.qmltypes
    DESTINATION ${SIGIL_QML_DIR}
)

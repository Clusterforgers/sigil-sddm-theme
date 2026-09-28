import QtQuick
import Sigil

// The live figure, from the `Sigil` plugin. If the plugin is not installed this file fails
// to load, and Figure.qml shows the still instead.
SigilItem {
    figure: Qt.resolvedUrl("../figure.json5")
}

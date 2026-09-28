import QtQuick
import QtQuick.Effects

// The figure as a still, answering the same calls as the live one with what a still can
// do: glow when typed at, gather and burst on Enter, flush red when refused.
Item {
    id: still

    signal detonated()

    property color ink: "#FBB929"
    property color blood: "#C0120C"
    property bool busy: false
    // 0 to 1: the light gathered by typing, and by the charge.
    property real glow: 0
    property real refused: 0

    function key() { glow = Math.min(1, glow + 0.25); cool.restart() }
    function backspace() { glow = Math.max(0, glow - 0.15) }
    function surge() {
        if (busy)
            return false
        busy = true
        charge.start()
        return true
    }
    function fail() {
        charge.stop()
        reform.start()
        refusal.restart()
    }

    Image {
        id: art
        anchors.fill: parent
        source: "../fallback.png"
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
        visible: false
    }

    MultiEffect {
        id: fx
        source: art
        anchors.fill: art
        brightness: 0.35 * still.glow
        // A halo of its own gold, stronger as light gathers.
        shadowEnabled: true
        shadowColor: still.ink
        shadowBlur: 0.5 + 0.5 * still.glow
        shadowOpacity: 0.25 + 0.75 * still.glow
        shadowHorizontalOffset: 0
        shadowVerticalOffset: 0
        blurEnabled: true
        blurMax: 48
        blur: 0
        colorization: still.refused
        colorizationColor: still.blood
        // The still breathes, so the screen never looks frozen.
        SequentialAnimation on opacity {
            loops: Animation.Infinite
            NumberAnimation { from: 1; to: 0.86; duration: 3200; easing.type: Easing.InOutSine }
            NumberAnimation { from: 0.86; to: 1; duration: 3200; easing.type: Easing.InOutSine }
        }
    }

    NumberAnimation { id: cool; target: still; property: "glow"; to: 0; duration: 1600; easing.type: Easing.InQuad }

    // Enter: drawing in on itself as the light gathers, then gone in a flash.
    SequentialAnimation {
        id: charge
        ParallelAnimation {
            NumberAnimation { target: still; property: "glow"; to: 1; duration: 1200; easing.type: Easing.InQuad }
            NumberAnimation { target: fx; property: "scale"; to: 0.96; duration: 1200; easing.type: Easing.InCubic }
        }
        ScriptAction { script: still.detonated() }
        ParallelAnimation {
            NumberAnimation { target: fx; property: "scale"; to: 1.7; duration: 900; easing.type: Easing.OutCubic }
            NumberAnimation { target: fx; property: "blur"; to: 1; duration: 900 }
            NumberAnimation { target: art; property: "opacity"; to: 0; duration: 900 }
        }
    }

    // Refused: back as it was, at once.
    ParallelAnimation {
        id: reform
        NumberAnimation { target: fx; property: "scale"; to: 1; duration: 350; easing.type: Easing.OutBack }
        NumberAnimation { target: fx; property: "blur"; to: 0; duration: 350 }
        NumberAnimation { target: art; property: "opacity"; to: 1; duration: 250 }
        NumberAnimation { target: still; property: "glow"; to: 0; duration: 350 }
        onFinished: still.busy = false
    }

    SequentialAnimation {
        id: refusal
        NumberAnimation { target: still; property: "refused"; to: 0.85; duration: 120 }
        NumberAnimation { target: still; property: "refused"; to: 0; duration: 1400; easing.type: Easing.InQuad }
    }
}

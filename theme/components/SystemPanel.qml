import QtQuick
import Sigil

// The machine, read like a table of correspondences: each line under the planet or element
// that suits it. Comes from the `Sigil` plugin; without it, this file fails to load and
// Main.qml simply leaves the panel out.
Column {
    id: panel

    property color ink: "#FBB929"
    property string family
    property string symbols: "Noto Sans Symbols"
    property real size: 15

    spacing: size * 0.95

    function bytes(b) {
        const gib = b / (1024 * 1024 * 1024)
        return gib >= 10 ? gib.toFixed(0) : gib.toFixed(1)
    }
    function duration(s) {
        const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60)
        return d > 0 ? d + " d " + h + " h" : h > 0 ? h + " h " + m + " m" : m + " m"
    }

    Line { sign: "☉"; label: "system"; value: SystemInfo.os }
    Line { sign: "☿"; label: "host"; value: SystemInfo.host }
    Line { sign: "♃"; label: "kernel"; value: SystemInfo.kernel }
    Line {
        sign: "♂"
        label: "mind"
        value: Math.round(SystemInfo.cpuLoad * 100) + "%"
            + (SystemInfo.temperature >= 0 ? "  ·  " + Math.round(SystemInfo.temperature) + "°" : "")
        detail: SystemInfo.cpu + (SystemInfo.cores > 0 ? "  ·  " + SystemInfo.cores + " threads" : "")
        fill: SystemInfo.cpuLoad
    }
    Line {
        sign: "☽"
        label: "memory"
        value: panel.bytes(SystemInfo.memoryUsed) + " / " + panel.bytes(SystemInfo.memoryTotal) + " GiB"
        fill: SystemInfo.memoryTotal > 0 ? SystemInfo.memoryUsed / SystemInfo.memoryTotal : 0
    }
    Line { sign: "♄"; label: "awake"; value: panel.duration(SystemInfo.uptime) }
    Line {
        visible: SystemInfo.battery >= 0
        sign: "🜂"
        label: "fire"
        value: SystemInfo.battery + "%" + (SystemInfo.charging ? "  ·  drawing" : "")
        fill: SystemInfo.battery / 100
    }

    // A sign, its name, what it reads; optionally a smaller line under it, and a fine rule
    // filled as far as `fill` says.
    component Line: Row {
        id: line
        property string sign
        property string label
        property string value
        property string detail
        property real fill: -1

        spacing: panel.size * 0.9

        Text {
            width: panel.size * 1.4
            text: line.sign + "︎"
            color: panel.ink
            font.family: panel.symbols
            font.pixelSize: panel.size * 1.25
            horizontalAlignment: Text.AlignHCenter
        }
        Column {
            spacing: panel.size * 0.25
            Row {
                spacing: panel.size * 0.7
                Text {
                    width: panel.size * 5.2
                    text: line.label
                    color: panel.ink
                    opacity: 0.55
                    font.family: panel.family
                    font.pixelSize: panel.size * 0.85
                    font.capitalization: Font.SmallCaps
                    font.letterSpacing: panel.size * 0.12
                    anchors.baseline: reading.baseline
                }
                Text {
                    id: reading
                    text: line.value
                    color: panel.ink
                    font.family: panel.family
                    font.pixelSize: panel.size
                    font.letterSpacing: panel.size * 0.04
                }
            }
            Text {
                visible: line.detail !== ""
                x: panel.size * 5.9
                text: line.detail
                color: panel.ink
                opacity: 0.55
                font.family: panel.family
                font.pixelSize: panel.size * 0.72
            }
            // The rule: faint for all of it, lit for the share in use.
            Item {
                visible: line.fill >= 0
                x: panel.size * 5.9
                width: panel.size * 9
                height: 1.5
                Rectangle { anchors.fill: parent; color: panel.ink; opacity: 0.18 }
                Rectangle {
                    height: parent.height
                    width: parent.width * Math.max(0, Math.min(1, line.fill))
                    color: panel.ink
                    Behavior on width { NumberAnimation { duration: 800; easing.type: Easing.OutCubic } }
                }
            }
        }
    }
}

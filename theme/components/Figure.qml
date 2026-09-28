import QtQuick

// The sigil: the live engine when the `Sigil` plugin loads, otherwise a still of it. The
// login works the same either way — a missing plugin or a GPU that will not cooperate must
// never be what stands between someone and their desktop.
Item {
    id: fig

    // Emitted when the surge explodes: the moment the password is handed over.
    signal detonated()

    // Canvas size and the outer ring's radius, in canvas units (`imagespin check`).
    readonly property real canvasWidth: 2000
    readonly property real canvasHeight: 1125
    readonly property real disc: 442.5
    // Where the figure is fitted. The item itself covers the screen, so the explosion's
    // flash can too; the figure sits in this part of it.
    property rect area: Qt.rect(0, 0, width, height)
    readonly property real fit: Math.min(area.width / canvasWidth, area.height / canvasHeight)
    readonly property real discRadius: disc * fit
    readonly property point discCenter: Qt.point(area.x + area.width / 2, area.y + area.height / 2)

    property color ink: "#FBB929"
    property color blood: "#C0120C"

    // The plugin is there, and the figure it loads on a thread of its own has arrived.
    readonly property bool plugin: engine.status === Loader.Ready
    readonly property bool live: plugin && engine.item.ready
    // Still arriving: a moment of bare background, which is less jarring than the still
    // showing and then being swapped out.
    readonly property bool loading: plugin && !engine.item.ready && engine.item.error === ""
    readonly property Item target: live ? engine.item : still
    // Whether the figure file asks for the system panel. The panel needs the plugin, so
    // without it there is none to ask for.
    readonly property bool systemInfo: live && engine.item.systemInfo
    // Seconds of the explosion to show before the password is checked: a password that works
    // ends the login screen at once. The still's burst is over in about this long too.
    readonly property real checkAfter: live ? engine.item.checkAfter : 0.5
    // The debug readout, if the figure file asks for one (`login.debug`). Without the plugin
    // there is nothing to measure, and the still says so for itself.
    readonly property bool debug: live && engine.item.debug
    readonly property string device: plugin ? engine.item.device : ""
    readonly property bool software: plugin && engine.item.software
    readonly property string stats: plugin ? engine.item.stats : ""

    function key() { target.key() }
    function backspace() { target.backspace() }
    // True if the surge started; false if one was already running.
    function surge() { return target.surge() }
    function fail() { target.fail() }

    Loader {
        id: engine
        anchors.fill: parent
        source: "SigilLive.qml"
        onLoaded: item.figureArea = Qt.binding(() => fig.area)
        onStatusChanged: if (status === Loader.Error)
            console.warn("sigil: live figure unavailable, showing the still")
    }

    FallbackFigure {
        id: still
        x: fig.area.x
        y: fig.area.y
        width: fig.area.width
        height: fig.area.height
        visible: !fig.live && !fig.loading
        ink: fig.ink
        blood: fig.blood
    }

    Connections {
        target: fig.target
        function onDetonated() { fig.detonated() }
    }
}

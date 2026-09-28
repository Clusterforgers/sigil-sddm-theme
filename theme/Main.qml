import QtQuick
import "components"

// The login screen: the sigil, and under it the name and the password written in symbols.
// Typing lights the figure's letters; Enter charges it, and the password is handed over
// the moment it explodes. A refused password snaps it back together in red.
Rectangle {
    id: root

    width: 1920
    height: 1080

    // theme.conf, with the look of the figure as the fallback.
    function conf(key, fallback) {
        const v = typeof config !== "undefined" ? config[key] : undefined
        return v === undefined || v === "" ? fallback : v
    }

    readonly property color ink: conf("ink", "#FBB929")
    readonly property color blood: conf("blood", "#C0120C")
    readonly property string refusedText: conf("refusedText", "the seal refuses")
    readonly property real unit: height / 1080
    // For the preview's demo.
    readonly property Item passwordField: password

    color: conf("background", "#0E0D0C")

    FontLoader { id: display; source: "fonts/Metamorphous-Regular.ttf" }
    FontLoader { source: "fonts/NotoSansSymbols-Regular.ttf" }
    FontLoader { source: "fonts/NotoSansSymbols2-Regular.ttf" }
    FontLoader { source: "fonts/NotoSansRunic-Regular.ttf" }

    // Everything under the figure is placed from where its outer ring ends.
    readonly property real discBottom: figure.y + figure.discCenter.y + figure.discRadius

    Figure {
        id: figure
        anchors.fill: parent
        // The upper part of the screen, leaving room for the name and password below.
        area: Qt.rect(0, 0, width, height * 0.84)
        ink: root.ink
        blood: root.blood
        onDetonated: checkAfter.restart()
    }

    Chooser {
        id: user
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.discBottom + 26 * root.unit
        model: userModel
        currentIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
        ink: root.ink
        family: display.name
        size: 22 * root.unit
        opacity: password.sealing ? 0 : 0.9
        Behavior on opacity { NumberAnimation { duration: 400 } }
    }

    PasswordSigil {
        id: password
        width: parent.width
        anchors.top: user.bottom
        anchors.topMargin: -6 * root.unit
        glyphSize: 34 * root.unit
        ink: root.ink
        blood: root.blood
        textFamily: display.name
        capsLock: keyboard.capsLock
        // Drawn up into the middle of the figure.
        sink: mapFromItem(figure, figure.discCenter.x, figure.discCenter.y)
        focus: true

        onKeyed: figure.key()
        onBackspaced: figure.backspace()
        onPreviousUser: user.step(-1)
        onNextUser: user.step(1)
        onSubmitted: root.submit()
    }

    Chooser {
        id: session
        anchors { left: parent.left; top: parent.top; margins: 36 * root.unit }
        model: sessionModel
        currentIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
        ink: root.ink
        family: display.name
        size: 17 * root.unit
        showArrows: true
        opacity: chrome.opacity
    }

    // Down the left side, level with the figure: `login.system_info` in the figure file, or
    // `programs.sigil-sddm.systemInfo` in NixOS. Needs the plugin; left out without it.
    Loader {
        active: figure.systemInfo
        anchors { left: parent.left; leftMargin: 44 * root.unit }
        y: figure.discCenter.y - height / 2
        source: "components/SystemPanel.qml"
        opacity: chrome.opacity
        onLoaded: {
            item.ink = Qt.binding(() => root.ink)
            item.family = Qt.binding(() => display.name)
            item.size = Qt.binding(() => 15 * root.unit)
        }
    }

    Clock {
        anchors { right: parent.right; top: parent.top; margins: 36 * root.unit }
        ink: root.ink
        family: display.name
        size: 40 * root.unit
        opacity: chrome.opacity
    }

    PowerBar {
        anchors { right: parent.right; bottom: parent.bottom; margins: 36 * root.unit }
        ink: root.ink
        family: display.name
        size: 16 * root.unit
        opacity: chrome.opacity
    }

    // The corners' shared opacity: always there, until Enter clears the screen for the
    // surge.
    QtObject {
        id: chrome
        property real opacity: password.sealing ? 0 : 0.75
        Behavior on opacity { NumberAnimation { duration: 900; easing.type: Easing.InOutQuad } }
    }

    // Enter: the symbols go up into the figure and it charges. The password is checked only
    // once it has exploded and flown apart for `login.check_after` seconds: a password that
    // works ends the login screen at once, so anything after that would never be seen.
    function submit() {
        password.sealing = true
        if (!figure.surge())
            handOver()
        else
            watchdog.restart()
    }

    function handOver() {
        watchdog.stop()
        checkAfter.stop()
        if (!password.sealing)
            return
        sddm.login(user.name, password.text, session.currentIndex)
    }

    Timer { id: checkAfter; interval: figure.checkAfter * 1000; onTriggered: root.handOver() }
    // Should the explosion never come, the login still goes ahead.
    Timer { id: watchdog; interval: 5000 + figure.checkAfter * 1000; onTriggered: root.handOver() }

    Connections {
        target: sddm
        function onLoginFailed() {
            figure.fail()
            password.refuse(root.refusedText)
        }
        function onLoginSucceeded() { darken.start() }
    }

    Rectangle {
        id: curtain
        anchors.fill: parent
        color: "black"
        opacity: 0
        NumberAnimation { id: darken; target: curtain; property: "opacity"; to: 1; duration: 400 }
    }

    // A click anywhere that is not a button puts the typing back in the password.
    TapHandler { onTapped: password.focusInput() }

    Component.onCompleted: password.focusInput()
}

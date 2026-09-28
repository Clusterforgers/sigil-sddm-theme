import QtQuick
import QtQuick.Window

// The theme outside SDDM, against stand-ins for what the greeter provides. The Loader
// creates Main.qml in this file's context, so the ids below are what it finds as `sddm`,
// `userModel` and the rest.
//
//   qml preview/Preview.qml
//
// The password is `sigil`; anything else is refused.
Window {
    width: 1920
    height: 1080
    visible: true
    title: "sigil — preview"
    color: "black"

    QtObject {
        id: sddm
        signal loginFailed()
        signal loginSucceeded()
        property bool canSuspend: true
        property bool canReboot: true
        property bool canPowerOff: true

        function login(user, password, session) {
            console.log("login", user, "session", session)
            verdict.ok = password === "sigil"
            verdict.restart()
        }
        function suspend() { console.log("suspend") }
        function reboot() { console.log("reboot") }
        function powerOff() { console.log("power off") }
    }

    // PAM takes a moment, and longer to say no.
    Timer {
        id: verdict
        property bool ok
        interval: ok ? 300 : 1200
        onTriggered: ok ? sddm.loginSucceeded() : sddm.loginFailed()
    }

    ListModel {
        id: userModel
        property int lastIndex: 0
        ListElement { name: "chris"; realName: "Chris" }
        ListElement { name: "guest"; realName: "" }
    }

    ListModel {
        id: sessionModel
        property int lastIndex: 0
        ListElement { name: "Hyprland" }
        ListElement { name: "Plasma (Wayland)" }
    }

    QtObject {
        id: keyboard
        property bool capsLock: false
    }

    QtObject {
        id: config
    }

    Loader {
        id: theme
        anchors.fill: parent
        source: "../theme/Main.qml"
        focus: true
    }

    // `qml preview/Preview.qml -- demo`: once the figure has drawn itself in, a refused
    // password and then the right one, typed from in here rather than by faking keys.
    SequentialAnimation {
        running: Qt.application.arguments.indexOf("demo") >= 0 && theme.status === Loader.Ready
        PauseAnimation { duration: 8000 }
        ScriptAction { script: theme.item.passwordField.insert("nope") }
        PauseAnimation { duration: 600 }
        ScriptAction { script: theme.item.passwordField.enter() }
        PauseAnimation { duration: 6000 }
        ScriptAction { script: theme.item.passwordField.insert("sigil") }
        PauseAnimation { duration: 600 }
        ScriptAction { script: theme.item.passwordField.enter() }
    }
}

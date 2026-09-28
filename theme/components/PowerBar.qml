import QtQuick

// Sleep, restart, shut down — whichever the system allows.
Row {
    id: bar

    property color ink
    property string family
    property real size: 16

    spacing: size * 1.6

    TextButton {
        visible: sddm.canSuspend
        text: "sleep"
        ink: bar.ink
        size: bar.size
        font.family: bar.family
        onClicked: sddm.suspend()
    }
    TextButton {
        visible: sddm.canReboot
        text: "restart"
        ink: bar.ink
        size: bar.size
        font.family: bar.family
        onClicked: sddm.reboot()
    }
    TextButton {
        visible: sddm.canPowerOff
        text: "shut down"
        ink: bar.ink
        size: bar.size
        font.family: bar.family
        onClicked: sddm.powerOff()
    }
}

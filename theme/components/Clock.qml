import QtQuick

Column {
    id: clock

    property color ink
    property string family
    property real size: 30
    property date now: new Date()

    spacing: size * 0.1

    Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: clock.now = new Date()
    }

    Text {
        anchors.right: parent.right
        text: Qt.formatTime(clock.now, "HH:mm")
        color: clock.ink
        font.family: clock.family
        font.pixelSize: clock.size
        font.letterSpacing: clock.size * 0.06
    }
    Text {
        anchors.right: parent.right
        text: Qt.formatDate(clock.now, "dddd d MMMM")
        color: clock.ink
        opacity: 0.7
        font.family: clock.family
        font.pixelSize: clock.size * 0.42
        font.capitalization: Font.SmallCaps
        font.letterSpacing: clock.size * 0.05
    }
}

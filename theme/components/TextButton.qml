import QtQuick

// A word that can be clicked: dim until the pointer is on it.
Text {
    id: b

    signal clicked()

    property color ink
    property real size: 16

    color: ink
    opacity: area.containsMouse ? 1 : 0.6
    font.pixelSize: size
    font.capitalization: Font.AllUppercase
    font.letterSpacing: size * 0.12
    Behavior on opacity { NumberAnimation { duration: 150 } }

    MouseArea {
        id: area
        anchors.fill: parent
        anchors.margins: -b.size * 0.4
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: b.clicked()
    }
}

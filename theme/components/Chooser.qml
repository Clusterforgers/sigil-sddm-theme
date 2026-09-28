import QtQuick

// One of a model's entries, shown by its real name if it has one, with arrows to step
// through the others when there are any. Used for the user and for the session.
Row {
    id: chooser

    property var model
    property int currentIndex: 0
    property color ink
    property string family
    property real size: 16
    property bool showArrows: names.count > 1
    readonly property QtObject entry: names.count ? names.objectAt(currentIndex) : null
    // What is shown, and the `name` role itself (the login name, for a user).
    readonly property string current: entry ? entry.label : ""
    readonly property string name: entry ? entry.name : ""

    function step(by) {
        if (names.count > 1)
            currentIndex = (currentIndex + by + names.count) % names.count
    }

    spacing: size * 0.8

    Instantiator {
        id: names
        model: chooser.model
        delegate: QtObject {
            required property var model
            readonly property string name: model.name || ""
            readonly property string label: model.realName || model.name || ""
        }
    }

    TextButton {
        visible: chooser.showArrows
        text: "‹"
        ink: chooser.ink
        size: chooser.size
        font.family: chooser.family
        onClicked: chooser.step(-1)
    }
    Text {
        text: chooser.current
        color: chooser.ink
        font.family: chooser.family
        font.pixelSize: chooser.size
        font.capitalization: Font.SmallCaps
        font.letterSpacing: chooser.size * 0.25
    }
    TextButton {
        visible: chooser.showArrows
        text: "›"
        ink: chooser.ink
        size: chooser.size
        font.family: chooser.family
        onClicked: chooser.step(1)
    }
}

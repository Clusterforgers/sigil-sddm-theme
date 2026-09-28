import QtQuick
import QtQuick.Effects
import "Glyphs.js" as Glyphs

// The password, written in symbols under the figure. The real text lives in a hidden
// TextInput, so typing, pasting and input methods all behave as usual; what shows is one
// random symbol per character, which flickers through a few others as it is struck and
// settles. Nothing shown depends on what was typed.
Item {
    id: root

    property color ink: "#FBB929"
    property color blood: "#C0120C"
    property color flash: "#FFF4E0"
    property string textFamily
    property real glyphSize: 34
    property bool capsLock: false
    // Enter was pressed: the symbols are drawn up into the figure and input stops.
    property bool sealing: false
    // Where they are drawn to, in this item's coordinates.
    property point sink: Qt.point(width / 2, -height * 4)

    readonly property string text: input.text
    readonly property bool typing: input.activeFocus

    signal keyed()
    signal backspaced()
    signal submitted()
    // Up and Down, for whoever is choosing the user.
    signal previousUser()
    signal nextUser()

    implicitHeight: glyphSize * 3.2

    function focusInput() { input.forceActiveFocus() }

    // Type `t` as though it were pasted, and press Enter: for the preview's demo, which
    // must not send keystrokes to whatever window happens to have focus.
    function insert(t) { input.insert(input.cursorPosition, t) }
    function enter() { input.accepted() }

    // Forget what was typed, symbols and all, without the embers.
    function clear() {
        slots.clear()
        input.text = ""
    }

    // The password was refused: shake, clear, and say so for a moment.
    function refuse(message) {
        sealing = false
        clear()
        refusal.text = message
        refusal.opacity = 1
        refusalFade.restart()
        shake.restart()
        focusInput()
    }

    TextInput {
        id: input
        width: 1
        height: 1
        opacity: 0
        focus: true
        echoMode: TextInput.Password
        selectByMouse: false
        readOnly: root.sealing
        onTextChanged: root.sync()
        onAccepted: if (text.length > 0 && !root.sealing) root.submitted()
        Keys.onEscapePressed: root.clear()
        Keys.onUpPressed: root.previousUser()
        Keys.onDownPressed: root.nextUser()
    }

    ListModel { id: slots }

    // Bring the symbols in line with the length of the text: one struck for each character
    // added, one blown out for each taken away.
    function sync() {
        const want = input.text.length
        while (slots.count > want) {
            const last = glyphs.itemAt(slots.count - 1)
            if (last && last.ember)
                last.ember()
            slots.remove(slots.count - 1)
            backspaced()
        }
        let added = 0
        while (slots.count < want) {
            const recent = []
            for (let i = Math.max(0, slots.count - 3); i < slots.count; i++)
                recent.push(slots.get(i).sym)
            const g = Glyphs.pick(recent)
            // A paste arrives all at once; stagger it so it still reads as being written.
            slots.append({ sym: g.text, family: g.family, delay: added * 40 })
            added++
            keyed()
        }
    }

    Item {
        id: field
        width: parent.width
        height: parent.height
        transform: Translate { id: jolt }

        Row {
            id: row
            spacing: root.glyphSize * 0.32
            height: root.glyphSize * 1.6
            // Centred by hand rather than by anchor, so it can glide as it grows.
            x: (field.width - width) / 2
            y: rule.y - height - root.glyphSize * 0.1
            Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }

            layer.enabled: true
            layer.effect: MultiEffect {
                shadowEnabled: true
                shadowColor: root.ink
                shadowBlur: 0.7
                shadowHorizontalOffset: 0
                shadowVerticalOffset: 0
                shadowOpacity: 0.9
            }

            Repeater {
                id: glyphs
                model: slots
                delegate: Glyph {}
            }

            // The caret: a point of light where the next symbol will fall.
            Text {
                id: caret
                visible: input.activeFocus && !root.sealing
                text: "•"
                color: root.ink
                font.pixelSize: root.glyphSize * 0.5
                height: row.height
                verticalAlignment: Text.AlignVCenter
                SequentialAnimation on opacity {
                    running: caret.visible
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.15; duration: 700; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 0.9; duration: 700; easing.type: Easing.InOutSine }
                }
            }
        }

        // The rule the symbols stand on: engraved, fading out at both ends.
        Rectangle {
            id: rule
            width: Math.min(root.width, root.glyphSize * 16)
            height: 1.5
            anchors.horizontalCenter: parent.horizontalCenter
            y: root.glyphSize * 2
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: "transparent" }
                GradientStop { position: 0.5; color: root.capsLock ? root.blood : root.ink }
                GradientStop { position: 1.0; color: "transparent" }
            }
            opacity: input.activeFocus ? 0.8 : 0.35
            Behavior on opacity { NumberAnimation { duration: 300 } }
        }

        Text {
            anchors.left: rule.right
            anchors.leftMargin: -root.glyphSize * 1.2
            anchors.bottom: rule.top
            anchors.bottomMargin: root.glyphSize * 0.3
            visible: root.capsLock
            text: "⇪"
            color: root.blood
            font.pixelSize: root.glyphSize * 0.55
        }

        Text {
            id: refusal
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: rule.bottom
            anchors.topMargin: root.glyphSize * 0.45
            color: root.blood
            opacity: 0
            font.family: root.textFamily
            font.pixelSize: root.glyphSize * 0.5
            font.capitalization: Font.AllUppercase
            font.letterSpacing: root.glyphSize * 0.14
            SequentialAnimation {
                id: refusalFade
                PauseAnimation { duration: 2000 }
                NumberAnimation { target: refusal; property: "opacity"; to: 0; duration: 600 }
            }
        }
    }

    // A refused password: a hard horizontal shake that dies away.
    SequentialAnimation {
        id: shake
        NumberAnimation { target: jolt; property: "x"; to: -root.glyphSize * 0.6; duration: 50 }
        NumberAnimation { target: jolt; property: "x"; to: root.glyphSize * 0.5; duration: 70 }
        NumberAnimation { target: jolt; property: "x"; to: -root.glyphSize * 0.35; duration: 70 }
        NumberAnimation { target: jolt; property: "x"; to: root.glyphSize * 0.2; duration: 80 }
        NumberAnimation { target: jolt; property: "x"; to: 0; duration: 90; easing.type: Easing.OutCubic }
    }

    // A symbol blown out by Backspace: a copy that drifts up and fades, left behind in the
    // field while the slot itself goes at once.
    Component {
        id: emberComponent
        Text {
            id: e
            color: root.ink
            font.pixelSize: root.glyphSize
            ParallelAnimation {
                running: true
                NumberAnimation { target: e; property: "y"; to: e.y - root.glyphSize * 1.2; duration: 550; easing.type: Easing.OutCubic }
                NumberAnimation { target: e; property: "opacity"; to: 0; duration: 550; easing.type: Easing.InQuad }
                NumberAnimation { target: e; property: "scale"; to: 0.6; duration: 550 }
                onFinished: e.destroy()
            }
        }
    }

    component Glyph: Text {
        id: g
        required property string sym
        required property string family
        required property int delay
        required property int index

        // Negative while waiting its turn in a paste; then counts the flickers.
        property int flips: -Math.round(delay / 45)

        text: sym
        font.family: family
        font.pixelSize: root.glyphSize
        height: row.height
        verticalAlignment: Text.AlignVCenter
        color: root.flash
        opacity: flips < 0 ? 0 : 1
        transform: Translate { id: drift }

        function ember() {
            const p = g.mapToItem(field, 0, 0)
            emberComponent.createObject(field, { x: p.x, y: p.y, text: g.text, "font.family": g.font.family, height: g.height, verticalAlignment: Text.AlignVCenter })
        }

        // Struck: flicker through a few other symbols, then settle on its own with a flash.
        Timer {
            interval: 45
            repeat: true
            running: true
            onTriggered: {
                g.flips++
                if (g.flips <= 0)
                    return
                if (g.flips > 5) {
                    stop()
                    g.text = g.sym
                    g.font.family = g.family
                    settle.start()
                } else {
                    const r = Glyphs.pick([])
                    g.text = r.text
                    g.font.family = r.family
                }
            }
        }

        ParallelAnimation {
            id: settle
            NumberAnimation { target: g; property: "scale"; from: 1.5; to: 1; duration: 260; easing.type: Easing.OutBack }
            ColorAnimation { target: g; property: "color"; from: root.flash; to: root.ink; duration: 500; easing.type: Easing.OutCubic }
        }

        // Enter: pulled up into the figure one after another, shrinking as they go.
        Connections {
            target: root
            function onSealingChanged() {
                if (!root.sealing)
                    return
                const p = g.mapToItem(root, g.width / 2, g.height / 2)
                pullX.to = root.sink.x - p.x
                pullY.to = root.sink.y - p.y
                pull.start()
            }
        }
        SequentialAnimation {
            id: pull
            PauseAnimation { duration: Math.max(0, g.index) * 35 }
            ParallelAnimation {
                NumberAnimation { id: pullX; target: drift; property: "x"; duration: 750; easing.type: Easing.InCubic }
                NumberAnimation { id: pullY; target: drift; property: "y"; duration: 750; easing.type: Easing.InCubic }
                NumberAnimation { target: g; property: "scale"; to: 0.25; duration: 750; easing.type: Easing.InCubic }
                NumberAnimation { target: g; property: "opacity"; to: 0; duration: 750; easing.type: Easing.InQuart }
            }
        }
    }
}

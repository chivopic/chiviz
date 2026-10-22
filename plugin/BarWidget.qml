import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.Ui
import qs.Commons
import qs.Commons as Commons

BarWidget {
  id: root
  moduleName: "chiv.chiviz"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  readonly property var sink: Pipewire.defaultAudioSink
  readonly property bool hasAudio: sink !== null && sink.audio && !sink.audio.muted && sink.audio.volume > 0

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰎈"
    fontSize: Style.font.body
    active: root.hasAudio
    tooltipText: "⚡ Chiviz Audio Visualizer\n• Left-click: Launch / Focus Chiviz\n• Right-click: Diagnostics (--doctor)\n• Middle-click: Floating Window"

    onPressed: function(btn) {
      if (!root.bar) return
      if (btn === Qt.RightButton) {
        root.bar.run("omarchy-launch-tui --app-id=chiviz-doctor chiviz --doctor")
      } else if (btn === Qt.MiddleButton) {
        root.bar.run("omarchy-launch-tui --app-id=chiviz-float chiviz")
      } else {
        root.bar.run("omarchy-launch-or-focus-tui --app-id=chiviz chiviz")
      }
    }
  }
}

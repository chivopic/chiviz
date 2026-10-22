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
    tooltipText: "⚡ Chiviz 音频可视化\n• 左键：唤醒 / 聚焦可视化窗口\n• 右键：运行系统自检诊断 (--doctor)\n• 中键：浮动独立小窗模式"

    onPressed: function(btn) {
      if (btn === Qt.RightButton) {
        Quickshell.execDetached(["kitty", "--class", "chiviz-doctor", "--hold", "-e", "chiviz", "--doctor"])
      } else if (btn === Qt.MiddleButton) {
        Quickshell.execDetached(["kitty", "--class", "chiviz-float", "-e", "chiviz"])
      } else {
        Quickshell.execDetached(["omarchy-launch-or-focus", "chiviz-visualizer", "kitty --class chiviz-visualizer -e chiviz"])
      }
    }
  }
}

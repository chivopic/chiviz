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

  function launch(mode) {
    var helperPath = decodeURIComponent(Qt.resolvedUrl("scripts/chiviz-launch").toString().replace(/^file:\/\//, ""))
    Quickshell.execDetached([helperPath, mode])
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󰎈"
    fontSize: Style.font.body
    active: root.hasAudio
    tooltipText: "⚡ Chiviz 音频可视化\n• 左键：唤醒 / 聚焦可视化窗口\n• 右键：运行系统自检诊断 (--doctor)\n• 中键：浮动独立小窗模式\n需先构建 chiviz 并安装 Kitty"

    onPressed: function(btn) {
      if (btn === Qt.RightButton) {
        root.launch("doctor")
      } else if (btn === Qt.MiddleButton) {
        root.launch("float")
      } else {
        root.launch("visualizer")
      }
    }
  }
}

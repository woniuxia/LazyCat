// Desktop Widget · 挂件 WebView 挂载入口
//
// 由挂件 WebView 加载 (`?view=widget-canvas`)，渲染 360×800 仪表盘。

import { createApp } from "vue";
import WidgetCanvas from "./components/WidgetCanvas.vue";
import "./styles/index.css";

export default function mount() {
  createApp(WidgetCanvas).mount("#app");
}

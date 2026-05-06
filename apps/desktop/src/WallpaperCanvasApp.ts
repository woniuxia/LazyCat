// Living Wallpaper · 信息层 canvas 挂载入口
//
// 由 hidden WebView 加载 (`?view=wallpaper-canvas`)，渲染 360×800 仪表盘后
// 通过 `wallpaper://canvas-ready` 通知后端可抓帧。
//
// 关联设计：docs/superpowers/specs/2026-05-05-living-wallpaper-design.md §7
// 关联实施：docs/superpowers/specs/2026-05-05-living-wallpaper-plan.md §2.1-2.3

import { createApp } from "vue";
import WallpaperCanvas from "./components/WallpaperCanvas.vue";
import "./styles/index.css";

export default function mount() {
  createApp(WallpaperCanvas).mount("#app");
}

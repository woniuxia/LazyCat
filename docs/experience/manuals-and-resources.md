# 手册与资源经验

适用范围：离线手册、静态资源、浏览器预览和合成壁纸资源。

关键词：`manuals`、`Puppeteer`、`VitePress`、`offline`、`local preview`

## 运行时资源必须离线可用

字体、Monaco、手册静态文件等不得依赖公网 CDN。构建前确认资源进入应用包，并在断网环境做最小冒烟。

## 离线手册接入

每个手册使用独立本地 HTTP 端口，避免 VitePress 绝对路径资源冲突；前端通过 `ManualPanel.vue` iframe 展示。新增手册优先源码构建，必要时用 Puppeteer 抓取中文静态产物，同时更新后端 known 列表和前端 sidebar。

打包态从 `resource_dir()/manuals` 解析，开发态回退仓库 `resources/manuals`。大量资源变更超过 100 文件时，提交前确认范围。

## 本地视觉预览

设计/布局问题优先复用 `scripts/start-server.ps1` / `stop-server.ps1` 展示独立原型或说明页，不自动启动产品 dev server。预览脚本是沟通工具，不应成为产品运行时依赖。

## 合成壁纸

壁纸合成涉及采集预览、资源路径与 Windows 桌面应用生命周期；实现时保持预览资源临时性，最终产物路径显式，不把采集临时文件当作长期数据。

**使用次数**：0

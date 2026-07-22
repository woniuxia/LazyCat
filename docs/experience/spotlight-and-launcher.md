# Spotlight 与启动入口经验

适用范围：Spotlight provider、预取缓存、命名快捷键、浏览器身份和资源管理器动作。

关键词：`Spotlight`、`provider`、`prefetch`、`hotkey`、`browser profiles`

## 动态数据使用 query-time provider

数据字典等动态大数据通过查询时 provider 接入，不全量常驻缓存。预取 provider 更新后按 provider 版本失效，不能只刷新 UI 列表而保留旧搜索缓存。

## 浏览器身份搜索与启动参数分离

搜索归一化、别名和排序放纯函数；浏览器身份启动参数按浏览器能力单独构造，不强行复用通用 launcher 参数，避免 profile 参数与普通命令行参数相互污染。

## 命名快捷键复用统一协议

新增工具快捷键优先复用 `registerNamedHotkey` 与 `hotkey-navigate`，并同步设置页冲突检测。新增 Tauri 窗口或窗口动作时同时检查 capability；窗口 label 缺权限会表现为二次触发无法隐藏等局部故障。

## 剪贴板路径动作

主呼出检测到有效本地路径时可优先打开资源管理器；检测逻辑与导航动作分离，不让普通文本误触文件系统动作。

**使用次数**：0

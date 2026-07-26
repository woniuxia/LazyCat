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

## 剪贴板建议使用判别式动作

同一剪贴板内容可以同时产生领域工具建议和通用参考动作。建议 payload 使用 `open-tool` / `open-reference-card` 判别字段，不能用虚构工具 ID 复用主窗口导航。现有高置信度工具建议保持首位，通用参考结果通过独立 searchFields 支持“参考、置顶、卡片”等查询。

**使用次数**：0

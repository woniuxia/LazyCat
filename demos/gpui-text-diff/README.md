# gpui-text-diff

用 [gpui](https://gpui.rs) + [gpui-component](https://github.com/longbridge/gpui-component) 搭建的文本对比 demo：

- 左右双栏多行输入（多行 Input 组件，预填示例文本）
- 行级差异计算：标准 LCS 动态规划；任一侧超过 4000 行时自动退化为前后缀对齐的快速模式
- 差异展示：绿色 = 新增行（仅 B 有），红色 = 删除行（仅 A 有），带两侧行号
- 按钮：对比 / 交换 A/B / 载入示例
- 固定浅色主题（`Theme::change(ThemeMode::Light, …)`）

## 运行

前置：Windows + Rust stable（MSVC 工具链）。

```powershell
cd demos/gpui-text-diff
cargo run
```

首次编译需要构建 gpui 全套依赖，耗时几分钟属正常。

## 已知边界（demo 定位）

- 仅做「统一视图」的行级对比，未做双栏对齐视图与词内高亮。
- 没有虚拟滚动：数千行的差异渲染不做优化（超大文本走快速模式后同样整体渲染）。
- 不处理空白差异选项、忽略大小写等高级 compare 配置。

## 目录说明

- `src/diff.rs`：纯逻辑 diff（无 UI 依赖，带单元测试）
- `src/main.rs`：gpui 窗口与界面
- `.cargo/config.toml`：本机代理与 git-fetch 设置，非必需，可按需删除
- `.reference/`：gpui-component v0.5.1 参考源码（仅开发期对照 API 用，不参与构建）

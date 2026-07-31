# Windows 10 中文标题乱码修复 - 验证指南

## 已完成的修改

### 1. 文件清单

- ✅ `apps/desktop/src-tauri/lazycat.manifest` - UTF-8 manifest 文件
- ✅ `apps/desktop/src-tauri/build.rs` - 构建脚本（嵌入 manifest）
- ✅ `apps/desktop/src-tauri/Cargo.toml` - 添加 embed-resource 依赖
- ✅ 提交到 Git: `58e6a19`

### 2. 技术方案

使用 Microsoft 官方 `activeCodePage` manifest 元素，在编译时嵌入 UTF-8 声明到可执行文件。

---

## 验证步骤

### 前置条件

需要在能够完整构建 Tauri 应用的环境中进行（当前环境因 Perl 模块缺失导致 OpenSSL 构建失败）。

### 步骤 1：构建应用

```powershell
# 方式 A：使用项目脚本（推荐）
.\scripts\build-tauri-win.ps1

# 方式 B：直接使用 Tauri CLI
cd apps\desktop
pnpm tauri build --bundles nsis
```

### 步骤 2：验证 manifest 嵌入

构建完成后，使用 Windows SDK 的 `mt.exe` 工具提取并检查 manifest：

```powershell
cd apps\desktop\src-tauri\target\release

# 提取 manifest
mt.exe -inputresource:lazycat-desktop.exe -out:extracted.manifest

# 检查 UTF-8 声明
Select-String -Path extracted.manifest -Pattern "activeCodePage"
```

**预期输出**应包含：

```xml
<activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
```

### 步骤 3：运行时测试

#### 测试 3.1：主窗口标题

1. 运行 `lazycat-desktop.exe`
2. 检查主窗口标题是否正确显示 **"Lazycat 懒猫"**

#### 测试 3.2：待办提醒弹窗

1. 打开"待办事项"工具
2. 创建一个提醒任务并触发
3. 检查弹窗标题是否正确显示 **"待办提醒"**

#### 测试 3.3：绿色包测试

1. 将 `lazycat-desktop.exe` 复制到干净的 Windows 10 测试机
2. 不安装，直接运行
3. 验证中文标题显示正常

---

## 预期结果

### ✅ 成功标志

- manifest 中包含 `activeCodePage: UTF-8` 声明
- Windows 10 1903+ 系统上中文标题显示正常
- NSIS 安装包和绿色包都能正确显示中文

### ⚠️ 已知限制

- **Windows 10 旧版本**（< 1903 / build 18362）：`activeCodePage` 声明会被忽略，可能仍有乱码
- **建议**：在文档中注明最低支持 Windows 10 1903（2019 年 5 月更新）

---

## 故障排查

### 问题 1：构建失败 - OpenSSL 错误

**症状**：`Can't locate Locale/Maketext/Simple.pm`

**原因**：Git Bash 的 Perl 缺少必要模块

**解决方案**：

```powershell
# 使用 PowerShell（非 Git Bash）执行构建
powershell -File .\scripts\build-tauri-win.ps1
```

### 问题 2：manifest 未嵌入

**检查**：

```powershell
mt.exe -inputresource:lazycat-desktop.exe -out:test.manifest
cat test.manifest
```

**可能原因**：

- `lazycat.manifest` 文件不存在或路径错误
- `embed-resource` crate 未正确安装
- 使用了 `cargo build` 而非 `tauri build`

### 问题 3：中文仍然乱码

**检查清单**：

1. 确认 Windows 版本 >= 10 1903 (build 18362)
   ```powershell
   winver  # 查看版本号
   ```
2. 确认 manifest 已正确嵌入（见步骤 2）
3. 确认使用的是新构建的 exe（检查文件修改时间）

---

## 后续优化建议

### 1. 文档更新

在用户手册中添加系统要求：

```markdown
## 系统要求

- Windows 10 1903 或更高版本（推荐）
- 旧版本 Windows 10 可能出现中文显示问题
```

### 2. 版本检测（可选）

在应用启动时检测 Windows 版本，对旧版本显示提示：

```rust
// 示例代码（可选实现）
if windows_version < 18362 {
    show_warning("建议升级到 Windows 10 1903 以获得最佳体验");
}
```

### 3. 备选方案（如果 manifest 方案不够）

如果部分系统仍有问题，可考虑：

- 使用 `SetWindowTextW` 显式设置 Unicode 标题
- 在 Tauri 配置中使用 Unicode 转义序列

---

## 参考资料

- [Microsoft Docs: UTF-8 code pages](https://docs.microsoft.com/en-us/windows/apps/design/globalizing/use-utf8-code-page)
- [Application Manifests](https://docs.microsoft.com/en-us/windows/win32/sbscs/application-manifests)
- [embed-resource crate](https://crates.io/crates/embed-resource)

---

**验证完成后，请删除本文件或移至 `docs/` 目录。**

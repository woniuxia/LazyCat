# 上线包打包页面体验优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将上线包打包页改造成响应式前后端双栏工作台，支持多行 PowerShell 命令编辑、常用命令一键复制，并把运行日志调整为白色卡片。

**Architecture:** 保持现有项目配置字段、IPC 和 Rust 执行链路不变。命令示例作为 `releasePackage.ts` 中的只读前端数据，`ReleasePackagePanel.vue` 负责响应式布局、浮层展示、剪贴板交互和日志视觉；现有运行态 composable 继续作为唯一日志与状态来源。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、CSS Grid

---

## 文件职责

- `apps/desktop/src/utils/releasePackage.ts`：维护上线包页面纯函数，并新增可测试的 PowerShell 常用命令示例数据。
- `apps/desktop/src/utils/releasePackage.test.ts`：锁定示例分类及 Java/Maven、复制、移动、退出码检查等关键内容。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：实现双栏工程卡片、多行命令编辑、示例复制和白色日志卡片。
- `apps/desktop/src/components/ReleasePackagePanel.test.ts`：用源码结构测试锁定关键 UI 行为和样式约束。
- `process.md`：记录本次多行脚本编辑和紧凑工作台的工程经验。

### Task 1: 建立可测试的 PowerShell 命令示例

**Files:**
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`

- [ ] **Step 1: 写入失败测试**

在 `releasePackage.test.ts` 的 import 中加入 `RELEASE_PACKAGE_COMMAND_EXAMPLES`，并在现有 `describe` 中加入：

```ts
it("provides copyable PowerShell examples for build environments and file operations", () => {
  expect(RELEASE_PACKAGE_COMMAND_EXAMPLES.map((example) => example.id)).toEqual([
    "java-maven-env",
    "maven-build",
    "copy-file",
    "copy-directory",
    "move-file",
    "move-directory",
  ]);

  const commands = Object.fromEntries(
    RELEASE_PACKAGE_COMMAND_EXAMPLES.map((example) => [example.id, example.command]),
  );
  expect(commands["java-maven-env"]).toContain("$env:JAVA_HOME");
  expect(commands["java-maven-env"]).toContain("$env:MAVEN_HOME");
  expect(commands["maven-build"]).toContain("$LASTEXITCODE");
  expect(commands["copy-file"]).toContain("Copy-Item");
  expect(commands["copy-directory"]).toContain("-Recurse");
  expect(commands["move-file"]).toContain("Move-Item");
  expect(commands["move-directory"]).toContain("Move-Item");
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test src/utils/releasePackage.test.ts
```

Expected: FAIL，提示 `RELEASE_PACKAGE_COMMAND_EXAMPLES` 未导出。

- [ ] **Step 3: 添加最小示例数据实现**

在 `releasePackage.ts` 的类型 import 后加入：

```ts
export interface ReleasePackageCommandExample {
  id: "java-maven-env" | "maven-build" | "copy-file" | "copy-directory" | "move-file" | "move-directory";
  title: string;
  description: string;
  command: string;
}

export const RELEASE_PACKAGE_COMMAND_EXAMPLES: readonly ReleasePackageCommandExample[] = [
  {
    id: "java-maven-env",
    title: "配置 Java 与 Maven 环境",
    description: "仅影响当前构建会话，请按本机安装路径修改。",
    command: String.raw`$env:JAVA_HOME = 'C:\Tools\Java\jdk-17'
$env:MAVEN_HOME = 'C:\Tools\apache-maven-3.9.9'
$env:Path = "$env:JAVA_HOME\bin;$env:MAVEN_HOME\bin;$env:Path"`,
  },
  {
    id: "maven-build",
    title: "执行 Maven 生产构建",
    description: "构建失败时立即退出，避免后续命令掩盖错误。",
    command: String.raw`mvn clean package -Pprod
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }`,
  },
  {
    id: "copy-file",
    title: "复制文件",
    description: "复制单个文件并覆盖同名目标。",
    command: String.raw`Copy-Item -LiteralPath '.\target\app.jar' -Destination '.\release\app.jar' -Force`,
  },
  {
    id: "copy-directory",
    title: "复制目录",
    description: "递归复制目录内容并覆盖同名文件。",
    command: String.raw`Copy-Item -LiteralPath '.\config' -Destination '.\release\config' -Recurse -Force`,
  },
  {
    id: "move-file",
    title: "移动文件",
    description: "移动单个文件到目标位置。",
    command: String.raw`Move-Item -LiteralPath '.\target\app.jar' -Destination '.\release\app.jar' -Force`,
  },
  {
    id: "move-directory",
    title: "移动目录",
    description: "移动整个目录到目标位置。",
    command: String.raw`Move-Item -LiteralPath '.\release' -Destination '.\deploy\release' -Force`,
  },
];
```

- [ ] **Step 4: 运行测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test src/utils/releasePackage.test.ts
```

Expected: PASS，`release package view helpers` 下所有用例通过。

- [ ] **Step 5: 提交示例数据**

```powershell
git add apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "feat(release-package): 添加常用 PowerShell 命令示例"
```

### Task 2: 实现紧凑双栏命令工作台与白色日志卡片

**Files:**
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`

- [ ] **Step 1: 写入失败的组件结构测试**

在 `ReleasePackagePanel.test.ts` 中加入：

```ts
it("uses responsive engineering cards and multiline command editors", () => {
  expect(source).toContain('class="engineering-grid"');
  expect(source).toContain('class="engineering-card frontend-card"');
  expect(source).toContain('class="engineering-card backend-card"');
  expect(source.match(/type="textarea"/g)).toHaveLength(2);
  expect(source.match(/:autosize="\{ minRows: 4, maxRows: 9 \}"/g)).toHaveLength(2);
  expect(source).toContain("同一 PowerShell 会话中顺序执行");
});

it("renders copyable command examples and a white log card", () => {
  expect(source).toContain("RELEASE_PACKAGE_COMMAND_EXAMPLES");
  expect(source).toContain("navigator.clipboard.writeText(command)");
  expect(source).toContain("常用示例");
  expect(source).toContain("CopyDocument");
  expect(source).toContain('class="release-package-log-card"');
  expect(source).toMatch(/\.release-package-log\s*\{[^}]*background:\s*#fff;/s);
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test src/components/ReleasePackagePanel.test.ts
```

Expected: FAIL，缺少 `engineering-grid`、多行输入和命令示例复制结构。

- [ ] **Step 3: 接入示例数据、复制交互和状态标签**

将图标 import 增加 `CopyDocument`，并从 `../utils/releasePackage` import `RELEASE_PACKAGE_COMMAND_EXAMPLES`。在现有 computed 区域加入：

```ts
const statusLabel = computed(() => ({
  idle: "未运行",
  running: "运行中",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已终止",
})[status.value]);

async function copyCommandExample(command: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(command);
    ElMessage.success("命令示例已复制");
  } catch (error) {
    showError(error);
  }
}
```

- [ ] **Step 4: 将工程表单替换为响应式双栏卡片**

保留现有字段及 `v-model`，将 `<el-form>` 内容替换为以下结构。前后端的示例浮层都读取同一份只读数据，复制按钮只调用 `copyCommandExample`：

```vue
<el-form label-position="top" class="release-package-form">
  <div class="basic-card">
    <el-form-item label="项目名称" required>
      <el-input v-model="draft.name" :disabled="running" placeholder="例如：订单管理系统" />
    </el-form-item>
  </div>

  <div class="engineering-grid">
    <section class="engineering-card frontend-card">
      <div class="engineering-heading">
        <span class="engineering-mark">F</span>
        <div><h3>前端工程</h3><p>构建前端资源并选择归档方式</p></div>
      </div>
      <el-form-item label="工程目录" required>
        <el-input v-model="draft.frontendProjectPath" :disabled="running" placeholder="前端工程绝对路径">
          <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseFrontendProject">选择</el-button></template>
        </el-input>
      </el-form-item>
      <div class="field-grid">
        <el-form-item label="产物路径" required>
          <el-input v-model="draft.frontendArtifactPath" :disabled="running" placeholder="相对工程目录" />
        </el-form-item>
        <el-form-item label="产物处理方式" required>
          <el-select v-model="draft.frontendArtifactMode" :disabled="running" class="full-width">
            <el-option label="直接复制目录" value="copy_directory" />
            <el-option label="压缩为 ZIP" value="zip_directory" />
          </el-select>
        </el-form-item>
      </div>
      <el-form-item required class="command-field">
        <template #label>
          <div class="command-label">
            <span>构建命令</span>
            <el-popover placement="bottom-end" :width="560" trigger="click">
              <template #reference><el-button type="primary" link size="small">常用示例</el-button></template>
              <div class="command-examples">
                <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example">
                  <div class="command-example-heading"><div><strong>{{ example.title }}</strong><p>{{ example.description }}</p></div><el-button :icon="CopyDocument" size="small" @click="copyCommandExample(example.command)">复制</el-button></div>
                  <pre>{{ example.command }}</pre>
                </article>
              </div>
            </el-popover>
          </div>
        </template>
        <el-input v-model="draft.frontendBuildCommand" type="textarea" :autosize="{ minRows: 4, maxRows: 9 }" resize="vertical" :disabled="running" placeholder="每行一条 PowerShell 命令，例如：pnpm build" />
        <div class="command-help">多行脚本会在同一 PowerShell 会话中顺序执行，环境变量可供后续命令使用。</div>
      </el-form-item>
    </section>

    <section class="engineering-card backend-card">
      <div class="engineering-heading">
        <span class="engineering-mark">B</span>
        <div><h3>后端工程</h3><p>构建服务端产物并合并归档</p></div>
      </div>
      <el-form-item label="工程目录" required>
        <el-input v-model="draft.backendProjectPath" :disabled="running" placeholder="后端工程绝对路径">
          <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseBackendProject">选择</el-button></template>
        </el-input>
      </el-form-item>
      <el-form-item label="产物路径" required>
        <el-input v-model="draft.backendArtifactPath" :disabled="running" placeholder="相对工程目录，可为文件或目录" />
      </el-form-item>
      <el-form-item required class="command-field">
        <template #label>
          <div class="command-label">
            <span>构建命令</span>
            <el-popover placement="bottom-end" :width="560" trigger="click">
              <template #reference><el-button type="primary" link size="small">常用示例</el-button></template>
              <div class="command-examples">
                <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example">
                  <div class="command-example-heading"><div><strong>{{ example.title }}</strong><p>{{ example.description }}</p></div><el-button :icon="CopyDocument" size="small" @click="copyCommandExample(example.command)">复制</el-button></div>
                  <pre>{{ example.command }}</pre>
                </article>
              </div>
            </el-popover>
          </div>
        </template>
        <el-input v-model="draft.backendBuildCommand" type="textarea" :autosize="{ minRows: 4, maxRows: 9 }" resize="vertical" :disabled="running" placeholder="每行一条 PowerShell 命令，例如：mvn clean package" />
        <div class="command-help">多行脚本会在同一 PowerShell 会话中顺序执行；外部工具失败后建议检查 $LASTEXITCODE。</div>
      </el-form-item>
    </section>
  </div>
</el-form>
```

- [ ] **Step 5: 将日志包装为带状态的白色卡片**

用以下结构替换原日志 `<section>`：

```vue
<section class="release-package-log-card">
  <header class="log-header">
    <div><strong>运行日志</strong><span>实时显示构建、归档与错误输出</span></div>
    <span class="log-status" :class="status">{{ statusLabel }}</span>
  </header>
  <div ref="logContainer" class="release-package-log" aria-live="polite" aria-label="打包日志">
    <div v-if="logs.length === 0" class="log-empty">暂无运行日志</div>
    <div v-for="(entry, index) in logs" :key="`${entry.runId}-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
      <span class="log-meta">[{{ entry.phase }}] [{{ entry.stream }}]</span>
      <span>{{ entry.line }}</span>
    </div>
  </div>
</section>
```

- [ ] **Step 6: 添加紧凑双栏、命令编辑器、示例浮层和日志样式**

删除旧 `.el-divider`、`.form-grid` 与深色日志规则，加入：

```css
.release-package-toolbar { padding: 10px 12px; border: 1px solid var(--lc-border, #e5e7eb); border-radius: 10px; background: #fff; }
.release-package-workspace { overflow: hidden; border: 1px solid var(--lc-border, #e5e7eb); border-radius: 12px; background: #fff; }
.release-package-projects { padding: 14px 12px; background: #fafbfc; }
.release-package-editor { padding: 16px; }
.basic-card { padding: 14px 16px 2px; border: 1px solid var(--lc-border, #e5e7eb); border-radius: 10px; background: #fff; }
.engineering-grid { display: grid; grid-template-columns: repeat(2, minmax(340px, 1fr)); gap: 14px; margin-top: 14px; }
.engineering-card { min-width: 0; padding: 16px; border: 1px solid var(--lc-border, #e5e7eb); border-radius: 12px; background: #fff; box-shadow: 0 6px 18px rgba(31, 41, 55, .045); }
.engineering-heading { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
.engineering-heading h3, .engineering-heading p { margin: 0; }
.engineering-heading h3 { font-size: 15px; }
.engineering-heading p { margin-top: 2px; color: var(--lc-text-secondary, #909399); font-size: 12px; }
.engineering-mark { display: grid; width: 30px; height: 30px; place-items: center; border-radius: 9px; color: var(--el-color-primary, #409eff); background: var(--el-color-primary-light-9, #ecf5ff); font-weight: 700; }
.field-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(150px, .75fr); gap: 12px; }
.command-label, .command-example-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; width: 100%; }
.command-field :deep(textarea) { font-family: var(--lc-font-mono, Consolas, monospace); line-height: 1.6; }
.command-help { margin-top: 6px; color: var(--lc-text-secondary, #909399); font-size: 12px; line-height: 1.5; }
.command-examples { max-height: min(560px, 70vh); overflow: auto; padding-right: 4px; }
.command-example + .command-example { margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--lc-border, #e5e7eb); }
.command-example-heading p { margin: 3px 0 0; color: var(--lc-text-secondary, #909399); font-size: 12px; }
.command-example pre { margin: 8px 0 0; overflow: auto; padding: 10px; border-radius: 8px; color: #303133; background: #f6f8fa; font: 12px/1.55 var(--lc-font-mono, Consolas, monospace); white-space: pre-wrap; }
.release-package-log-card { overflow: hidden; border: 1px solid var(--lc-border, #e5e7eb); border-radius: 12px; background: #fff; }
.log-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 14px; border-bottom: 1px solid var(--lc-border, #e5e7eb); }
.log-header div { display: flex; align-items: baseline; gap: 8px; }
.log-header span { color: var(--lc-text-secondary, #909399); font-size: 12px; }
.log-status { padding: 3px 9px; border-radius: 999px; background: #f2f3f5; }
.log-status.running { color: #1768ca; background: #eaf3ff; }
.log-status.succeeded { color: #20864a; background: #eaf8ef; }
.log-status.failed { color: #c43c3c; background: #fff0f0; }
.log-status.cancelled { color: #8a5b16; background: #fff7e8; }
.release-package-log { min-height: 180px; max-height: 320px; overflow: auto; padding: 12px 14px; color: #303133; background: #fff; font: 12px/1.6 var(--lc-font-mono, Consolas, monospace); }
.log-line.stderr { color: #d84b4b; }
.log-meta { color: #909399; }
@media (max-width: 1180px) { .engineering-grid { grid-template-columns: 1fr; } }
@media (max-width: 640px) { .field-grid { grid-template-columns: 1fr; gap: 0; } }
```

- [ ] **Step 7: 运行组件测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test src/components/ReleasePackagePanel.test.ts
```

Expected: PASS，组件结构测试全部通过。

- [ ] **Step 8: 联合运行上线包前端测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts
```

Expected: PASS，无警告和未处理异常。

- [ ] **Step 9: 提交页面实现**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat(release-package): 优化多命令打包工作台"
```

### Task 3: 全量验证并沉淀经验

**Files:**
- Modify: `process.md`

- [ ] **Step 1: 运行类型检查**

Run:

```powershell
pnpm typecheck
```

Expected: exit code 0，无 TypeScript 错误。

- [ ] **Step 2: 运行渲染层构建**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: exit code 0，Vite 构建完成；若首次出现 `spawn EPERM`，按项目规范重试一次。

- [ ] **Step 3: 运行前端测试集**

Run:

```powershell
pnpm test
```

Expected: exit code 0，全部前端单元测试通过。

- [ ] **Step 4: 检查补丁质量**

Run:

```powershell
git diff --check
```

Expected: 无输出，exit code 0。

- [ ] **Step 5: 在 process.md 记录本次经验**

追加以下条目，并把实际验证结果替换为本轮真实结果：

```markdown
## 2026-07-19: 上线包多命令编辑与紧凑工作台

**场景**: 上线包打包需要编辑多行 PowerShell 构建脚本，同时保持前后端配置可对照并提升日志可读性。
**解决**:
1. 保持后端单段 PowerShell 脚本语义，前端使用等宽多行编辑器显式呈现同会话执行能力。
2. 前后端工程使用响应式双栏卡片，宽度不足时退化为单列，不改变现有配置模型。
3. 常用命令以可测试的只读数据维护，通过浮层提供 Java/Maven、复制和移动示例的一键复制。
4. 日志改为白色卡片，用深灰正文、灰色元信息和红色错误维持层级。
**关键点**:
- 多行脚本不能在前端按行拆分，否则环境变量和 PowerShell 语句块语义会被破坏。
- 外部构建工具的非零退出码可能被后续命令掩盖，示例应在关键命令后检查 `$LASTEXITCODE`。
**涉及文件**:
- `apps/desktop/src/utils/releasePackage.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
**验证**:
- 上线包相关前端测试
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0
```

- [ ] **Step 6: 提交经验记录**

```powershell
git add process.md
git commit -m "docs(process): 记录上线包多命令编辑经验"
```

- [ ] **Step 7: 检查最终工作区状态和提交记录**

Run:

```powershell
git status --short
git log -4 --oneline
```

Expected: 工作区干净；最近提交依次包含设计文档、命令示例、页面实现和经验记录。

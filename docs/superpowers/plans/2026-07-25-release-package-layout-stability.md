# 上线包配置布局稳定性优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让上线包配置页在打包类型和认证方式切换时保持稳定的列宽、字段顺序和远程目标位置。

**Architecture:** 保留现有数据模型与条件真值，只调整 `ReleasePackagePanel.vue` 的布局边界：项目基础区使用显式固定首列，服务器区拆成认证分区和远程目标分区。用源结构回归测试锁定 CSS 轨道与模板容器，避免条件字段再次直接参与同一自动网格。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、CSS Grid。

---

### Task 1: 写布局结构回归测试（RED）

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Test: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 添加失败测试**

在“渲染互斥打包类型和类型字段”测试之后加入：

```ts
it("keeps conditional configuration inside stable layout sections", () => {
  expect(source).toMatch(
    /\.project-basics-grid\s*\{[^}]*grid-template-columns:\s*minmax\(240px,\s*320px\)\s+minmax\(0,\s*1fr\);/su,
  );
  expect(source).not.toMatch(/\.project-basics-grid\s*\{[^}]*auto-fit/su);

  expect(source).toContain('class="server-config-section server-auth-section"');
  expect(source).toContain('class="server-auth-details"');
  expect(source).toContain('class="private-key-config-grid"');
  expect(source).toContain('class="server-config-section server-target-section"');
  expect(source).toContain('class="server-target-grid"');

  const authDetailsStart = source.indexOf('class="server-auth-details"');
  const targetSectionStart = source.indexOf('class="server-config-section server-target-section"');
  expect(authDetailsStart).toBeGreaterThan(-1);
  expect(targetSectionStart).toBeGreaterThan(authDetailsStart);
  expect(source.slice(targetSectionStart)).toContain('label="前端远程目录"');
  expect(source.slice(targetSectionStart)).toContain('label="后端远程文件"');

  expect(source).toMatch(
    /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);/su,
  );
  expect(source).toMatch(
    /\.server-target-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
  );

  const tabletStyles = source.slice(source.indexOf("@media (max-width: 960px)"));
  expect(tabletStyles).toMatch(
    /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
  );
  const mobileStyles = source.slice(source.indexOf("@media (max-width: 640px)"));
  expect(mobileStyles).toMatch(
    /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*1fr;/su,
  );
  expect(mobileStyles).toMatch(/\.server-target-grid\s*\{[^}]*grid-template-columns:\s*1fr;/su);
});
```

- [ ] **Step 2: 确认 RED**

运行：

```powershell
pnpm test -- apps/desktop/src/components/ReleasePackagePanel.test.ts
```

预期：FAIL，原因是当前项目基础区仍使用 auto-fit，模板也没有新的认证/目标容器。若出现解析错误，先修正测试语法后重跑，不能跳过失败验证。

- [ ] **Step 3: 提交测试变更**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "test(release-package): 锁定配置布局结构"
```

### Task 2: 将服务器字段分成固定认证区和远程目标区

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue:240-322`

- [ ] **Step 1: 替换 server-config-body 内部结构**

保留外层折叠面板，把当前扁平的 server-config-grid 替换为以下结构。下面的密码模式面板包含现有选择器、操作按钮、提示、失效状态和摘要的完整内容：

```vue
<div class="server-config-body">
  <section class="server-config-section server-auth-section">
    <div class="server-config-section-heading">
      <div>
        <strong>连接认证</strong>
        <span>选择连接凭据，切换时只更新认证详情</span>
      </div>
    </div>
    <div class="server-auth-type-row">
      <el-form-item label="认证方式" required>
        <el-radio-group v-model="draft.sshAuthType" :disabled="running" class="auth-type-group">
          <el-radio-button value="password">账户密码</el-radio-button>
          <el-radio-button value="private_key">私钥文件</el-radio-button>
        </el-radio-group>
      </el-form-item>
    </div>
    <div class="server-auth-details">
      <div v-if="draft.sshAuthType === 'password'" class="server-auth-details-panel password-auth-panel">
        <el-form-item label="密码库凭据" required class="vault-credential-field">
          <div class="vault-credential-picker">
            <el-select
              v-model="draft.vaultEntryId"
              :disabled="running"
              :loading="vaultOptionsLoading"
              filterable
              clearable
              class="full-width"
              placeholder="选择服务器凭据"
            >
              <el-option
                v-for="option in vaultServerOptions"
                :key="option.id"
                :label="vaultCredentialLabel(option)"
                :value="option.id"
                :disabled="!option.complete"
              />
            </el-select>
            <el-button :icon="Refresh" :loading="vaultOptionsLoading" :disabled="running" @click="loadVaultServerOptions">刷新</el-button>
            <el-button :disabled="running" @click="openVault">密码管理</el-button>
          </div>
          <p class="vault-credential-hint">密码由密码库提供，上线包配置只保存凭据引用，不保存或展示服务器密码。</p>
          <div v-if="vaultBindingInvalid" class="vault-binding-invalid" role="alert">
            绑定的密码库凭据已失效，请重新选择
          </div>
          <div v-else-if="selectedVaultCredential" class="vault-credential-summary">
            <div>
              <span>服务器地址</span>
              <code>{{ selectedVaultCredential.address }}</code>
            </div>
            <div>
              <span>SSH 端口</span>
              <code>{{ selectedVaultCredential.port }}</code>
            </div>
            <div>
              <span>SSH 用户名</span>
              <code>{{ selectedVaultCredential.account }}</code>
            </div>
          </div>
        </el-form-item>
      </div>
      <div v-else class="server-auth-details-panel private-key-auth-panel">
        <div class="private-key-config-grid">
          <el-form-item label="服务器地址" required>
            <el-input v-model="draft.sshHost" :disabled="running" placeholder="例如：10.0.0.8" />
          </el-form-item>
          <el-form-item label="SSH 端口" required>
            <el-input-number v-model="draft.sshPort" :disabled="running" :min="1" :max="65535" controls-position="right" class="full-width" />
          </el-form-item>
          <el-form-item label="SSH 用户名" required>
            <el-input v-model="draft.sshUsername" :disabled="running" placeholder="例如：deploy" />
          </el-form-item>
          <el-form-item label="私钥文件" required class="private-key-file-field">
            <el-input v-model="draft.sshPrivateKeyPath" :disabled="running" placeholder="选择 OpenSSH 私钥文件" readonly>
              <template #append>
                <el-button :icon="Document" :disabled="running" @click="choosePrivateKey">选择私钥</el-button>
              </template>
            </el-input>
          </el-form-item>
        </div>
      </div>
    </div>
  </section>

  <section class="server-config-section server-target-section">
    <div class="server-config-section-heading">
      <div>
        <strong>远程目标</strong>
        <span>认证方式切换不会改变目标位置</span>
      </div>
    </div>
    <div class="server-target-grid">
      <el-form-item label="前端远程目录" required>
        <el-input v-model="draft.frontendRemoteDir" :disabled="running" placeholder="例如：/srv/portal/web" />
      </el-form-item>
      <el-form-item label="后端远程文件" required>
        <el-input v-model="draft.backendRemotePath" :disabled="running" placeholder="例如：/srv/portal/app.jar" />
      </el-form-item>
    </div>
  </section>
</div>
```

删除旧的 server-config-span-2 类使用；不要保留重复的认证方式、服务器地址、用户名、私钥或远程目标节点。

- [ ] **Step 2: 运行测试检查模板阶段**

```powershell
pnpm test -- apps/desktop/src/components/ReleasePackagePanel.test.ts
```

预期：若仍失败，失败应集中在尚未替换的 CSS 轨道断言，不应出现 Vue 模板解析错误或字段重复。

- [ ] **Step 3: 提交模板结构**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue
git commit -m "fix(release-package): 拆分认证与远程目标布局"
```

### Task 3: 添加稳定列轨道和响应式样式

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue:1360-1435,1643-1668`

- [ ] **Step 1: 替换基础区和服务器分区样式**

使用以下规则，删除旧的 server-config-grid 和 server-config-span-2 规则；保留 Vault picker、摘要和认证按钮的既有规则：

```css
.project-basics-grid {
  display: grid;
  grid-template-columns: minmax(240px, 320px) minmax(0, 1fr);
  gap: 14px;
}
.server-config-body {
  display: grid;
  gap: 18px;
  padding: 0 16px 2px;
  border-top: 1px solid #ebeef5;
}
.server-config-section {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.server-config-section-heading {
  display: grid;
  gap: 2px;
  padding-top: 2px;
}
.server-config-section-heading strong {
  color: #303133;
  font-size: 13px;
}
.server-config-section-heading span {
  color: #606266;
  font-size: 12px;
  line-height: 1.45;
}
.server-auth-type-row {
  width: min(320px, 100%);
}
.server-auth-details,
.server-auth-details-panel {
  min-width: 0;
}
.private-key-config-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0 12px;
}
.private-key-file-field {
  grid-column: 1 / -1;
}
.server-target-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0 12px;
}
```

- [ ] **Step 2: 按断点降级**

在 960px 断点加入：

```css
.private-key-config-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.server-target-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
```

在 640px 断点加入：

```css
.project-basics-grid {
  grid-template-columns: 1fr;
  gap: 0;
}
.server-auth-type-row {
  width: 100%;
}
.private-key-config-grid,
.server-target-grid {
  grid-template-columns: 1fr;
}
.private-key-file-field {
  grid-column: auto;
}
```

删除旧 server-config-grid 和相关断点覆盖；保留 vault-credential-picker 的移动端换行规则。

- [ ] **Step 3: 运行定向测试确认 GREEN**

```powershell
pnpm test -- apps/desktop/src/components/ReleasePackagePanel.test.ts
```

预期：ReleasePackagePanel 测试全部通过，无 Vue 模板解析或 CSS 正则断言错误。

- [ ] **Step 4: 提交样式变更**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "fix(release-package): 稳定配置区响应式列宽"
```

### Task 4: 完成全量验证和视觉冒烟

**Files:**

- Verify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Verify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 运行前端类型检查**

```powershell
pnpm typecheck
```

预期：exit code 0，无新增 TypeScript 错误。

- [ ] **Step 2: 构建渲染层**

```powershell
pnpm --filter @lazycat/desktop build:web
```

预期：exit code 0，Vue 模板和 CSS 编译成功。

- [ ] **Step 3: 检查差异范围**

```powershell
git diff --check
git status --short
git diff --stat HEAD~2..HEAD
```

预期：无空白错误；本次实现只涉及上线包组件及测试，不包含 IPC、Rust、数据库或无关文件。

- [ ] **Step 4: 做两组最小视觉冒烟**

1. 切换“本地归档 / 上传服务器”时，打包类型控件的左边界和宽度不变；小屏按单列显示。
2. 在服务器上传中切换“账户密码 / 私钥文件”时，认证方式和“远程目标”分区位置保持；Vault 失效提示、私钥文件和移动端换行不被裁切。

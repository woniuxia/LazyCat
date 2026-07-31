# 请求转发规则表单简化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将请求转发规则编辑表单改为全字段可见的紧凑分组布局，移除编号卡片和重复说明，同时保持全部既有行为。

**Architecture:** 仅调整 Vue 表单组件的模板分组和 scoped CSS，不改变 props、emits、计算属性或父弹窗。沿用现有源码结构测试锁定信息层级、响应式断点与关键交互标记。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、CSS Grid

---

### Task 1: 锁定紧凑分组结构

**Files:**

- Modify: `apps/desktop/src/components/RequestForwardPanel.test.ts`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] **Step 1: 写入失败测试**

在现有 `RequestForwardPanel source structure` 测试组中增加：

```ts
it("uses an unnumbered compact rule form with side-by-side endpoints", () => {
  expect(formSource).toContain('class="form-identity"');
  expect(formSource).toContain('class="form-endpoints"');
  expect(formSource).toContain('class="form-group__title">本地监听');
  expect(formSource).toContain('class="form-group__title">转发目标');
  expect(formSource).toContain('class="form-group__title">采集选项');
  expect(formSource).not.toContain('class="form-section__heading"');
  expect(formSource).not.toMatch(/<span>0[1-4]<\/span>/);
  expect(formSource).toMatch(
    /\.form-endpoints\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/s,
  );
  expect(formSource).toMatch(
    /@media \(max-width: 680px\)[\s\S]*?\.form-endpoints\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/,
  );
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm test src/components/RequestForwardPanel.test.ts`

Expected: FAIL，提示 `form-identity` 或 `form-endpoints` 不存在。

- [ ] **Step 3: 提交测试阶段改动**

当前测试文件包含用户已有改动，因此不单独提交测试红灯阶段，避免把未完成或不属于本任务的内容拆入提交；继续在同一工作区完成最小实现。

### Task 2: 实现紧凑表单布局

**Files:**

- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] **Step 1: 重组模板**

将四个 `.form-section` 卡片改成以下完整模板，字段节点保持原有属性和事件：

```vue
<el-form class="rule-form" label-position="top" @submit.prevent>
  <div class="form-identity">
    <div class="form-grid form-grid--identity">
      <el-form-item :error="errors?.name">
        <template #label>
          <span class="field-label">规则名称
            <el-tooltip content="用于在左侧规则列表中快速定位，最多 80 个字符。" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="规则名称提示"><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </template>
        <el-input
          :model-value="modelValue.name"
          :disabled="readonly || disabled"
          maxlength="80"
          show-word-limit
          placeholder="例如：本地 API 转发"
          @update:model-value="update('name', $event)"
        />
      </el-form-item>
      <el-form-item>
        <template #label>
          <span class="field-label">协议
            <el-tooltip :content="protocolTip" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="协议提示"><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </template>
        <el-select
          :model-value="modelValue.protocol"
          :disabled="persisted || readonly || disabled"
          @update:model-value="update('protocol', $event)"
        >
          <el-option label="HTTP" value="http" />
          <el-option label="TCP" value="tcp" />
          <el-option label="UDP" value="udp" />
        </el-select>
      </el-form-item>
    </div>
  </div>

  <div class="form-endpoints">
    <section class="form-group">
      <h3 class="form-group__title">本地监听</h3>
      <div class="form-grid">
        <el-form-item :error="errors?.bindHost">
          <template #label>
            <span class="field-label">监听地址
              <el-tooltip content="LazyCat 接收流量的本地 IP。使用 127.0.0.1 或 ::1 时仅允许本机访问。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="监听地址提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input
            :model-value="modelValue.bindHost"
            :disabled="readonly || disabled"
            placeholder="127.0.0.1 或 ::1"
            @update:model-value="update('bindHost', $event)"
          />
        </el-form-item>
        <el-form-item :error="errors?.listenPort">
          <template #label>
            <span class="field-label">监听端口
              <el-tooltip content="LazyCat 在本机占用并接收流量的端口，范围为 1 到 65535。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="监听端口提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input-number
            :model-value="modelValue.listenPort"
            :disabled="readonly || disabled"
            :min="1"
            :max="65535"
            controls-position="right"
            @update:model-value="update('listenPort', $event ?? 0)"
          />
        </el-form-item>
      </div>
      <div v-if="exposedListener" class="exposure-warning" role="alert">
        <strong>当前监听地址可被其他设备访问</strong>
        <span>请确认所在网络可信，并在系统防火墙中限制不必要的入站访问。</span>
      </div>
    </section>

    <section class="form-group">
      <h3 class="form-group__title">转发目标</h3>
      <el-form-item v-if="modelValue.protocol === 'http'" :error="errors?.targetUrl">
        <template #label>
          <span class="field-label">目标 URL
            <el-tooltip content="仅支持 HTTP/HTTPS 基础地址，不包含查询参数或片段。请求路径会追加到该地址。" placement="top">
              <el-icon class="field-tip" tabindex="0" aria-label="目标 URL 提示"><QuestionFilled /></el-icon>
            </el-tooltip>
          </span>
        </template>
        <el-input
          :model-value="modelValue.targetUrl ?? ''"
          :disabled="readonly || disabled"
          placeholder="https://example.com/api"
          @update:model-value="update('targetUrl', $event)"
        />
      </el-form-item>
      <div v-else class="form-grid">
        <el-form-item :error="errors?.targetHost">
          <template #label>
            <span class="field-label">目标主机
              <el-tooltip content="接收转发流量的目标 IP 或域名，不包含端口。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="目标主机提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input
            :model-value="modelValue.targetHost ?? ''"
            :disabled="readonly || disabled"
            placeholder="192.168.1.10 或 db.internal"
            @update:model-value="update('targetHost', $event)"
          />
        </el-form-item>
        <el-form-item :error="errors?.targetPort">
          <template #label>
            <span class="field-label">目标端口
              <el-tooltip content="目标服务实际监听的端口，范围为 1 到 65535。" placement="top">
                <el-icon class="field-tip" tabindex="0" aria-label="目标端口提示"><QuestionFilled /></el-icon>
              </el-tooltip>
            </span>
          </template>
          <el-input-number
            :model-value="modelValue.targetPort"
            :disabled="readonly || disabled"
            :min="1"
            :max="65535"
            controls-position="right"
            @update:model-value="update('targetPort', $event)"
          />
        </el-form-item>
      </div>
    </section>
  </div>

  <section v-if="modelValue.protocol === 'http'" class="form-group form-group--capture">
    <h3 class="form-group__title">采集选项</h3>
    <div class="capture-options">
      <el-checkbox
        :model-value="modelValue.captureHttpHeaders"
        :disabled="readonly || disabled"
        @update:model-value="update('captureHttpHeaders', Boolean($event))"
      >
        <span class="capture-option-label">采集请求与响应头
          <el-tooltip content="在日志详情中保留脱敏后的 HTTP 请求头和响应头。" placement="top">
            <el-icon class="field-tip" tabindex="0" aria-label="HTTP 头采集提示" @click.stop><QuestionFilled /></el-icon>
          </el-tooltip>
        </span>
      </el-checkbox>
      <el-checkbox
        :model-value="modelValue.captureHttpBody"
        :disabled="readonly || disabled"
        @update:model-value="update('captureHttpBody', Boolean($event))"
      >
        <span class="capture-option-label">采集请求与响应正文预览
          <el-tooltip content="在日志详情中保留有限长度的正文预览，可能包含业务数据，请按需开启。" placement="top">
            <el-icon class="field-tip" tabindex="0" aria-label="HTTP 正文采集提示" @click.stop><QuestionFilled /></el-icon>
          </el-tooltip>
        </span>
      </el-checkbox>
    </div>
  </section>
</el-form>
```

- [ ] **Step 2: 实现紧凑样式**

将 scoped 样式替换为以下完整内容：

```css
.rule-form {
  display: grid;
  gap: 14px;
}
.form-identity {
  min-width: 0;
}
.form-endpoints {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 20px;
}
.form-group {
  min-width: 0;
  padding-top: 12px;
  border-top: 1px solid #e1e5ea;
}
.form-group__title {
  margin: 0 0 10px;
  color: #526175;
  font-size: 14px;
  font-weight: 700;
}
.form-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 132px;
  gap: 10px;
}
.form-grid--identity {
  grid-template-columns: minmax(0, 1fr) 180px;
}
.rule-form :deep(.el-form-item) {
  margin-bottom: 10px;
}
.rule-form :deep(.el-form-item__label),
.rule-form :deep(.el-input__inner),
.rule-form :deep(.el-select__placeholder),
.rule-form :deep(.el-input-number .el-input__inner),
.rule-form :deep(.el-checkbox__label) {
  font-size: 16px;
}
.rule-form :deep(.el-select),
.rule-form :deep(.el-input-number) {
  width: 100%;
}
.field-label,
.capture-option-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.field-tip {
  color: #657386;
  cursor: help;
  font-size: 16px;
}
.field-tip:hover {
  color: var(--el-color-primary, #409eff);
}
.field-tip:focus-visible {
  border-radius: 50%;
  outline: 2px solid var(--el-color-primary, #409eff);
  outline-offset: 1px;
}

.exposure-warning {
  display: grid;
  gap: 3px;
  margin: -2px 0 0;
  padding: 8px 10px;
  border-left: 3px solid #d58a16;
  background: #fff8e8;
  color: #70490b;
  font-size: 14px;
  line-height: 1.45;
}

.capture-options {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 20px;
}

@media (max-width: 680px) {
  .form-endpoints {
    grid-template-columns: minmax(0, 1fr);
    gap: 14px;
  }
}

@media (max-width: 480px) {
  .form-grid,
  .form-grid--identity {
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
  }
}
```

- [ ] **Step 3: 运行针对性测试确认通过**

Run: `pnpm test src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts`

Expected: 两个测试文件全部通过。

### Task 3: 完成静态与构建验证

**Files:**

- Verify: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- Verify: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] **Step 1: 执行类型检查**

Run: `pnpm typecheck`

Expected: exit code 0，无 TypeScript/Vue 类型错误。

- [ ] **Step 2: 执行渲染层构建**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: exit code 0，Vite 构建完成。

- [ ] **Step 3: 检查补丁质量和范围**

Run: `git diff --check`

Expected: exit code 0，无空白错误。

检查 `git diff -- apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue apps/desktop/src/components/RequestForwardPanel.test.ts`，确认未改变字段绑定、禁用条件、Tooltip、安全警告、协议分支和父弹窗操作。

- [ ] **Step 4: 提交实现**

由于 `RequestForwardPanel.test.ts` 已包含用户的未提交改动，提交前只暂存本任务对应的文件/补丁，并确保不把无关工作区改动带入提交。提交信息使用：

```text
feat(request-forward): 简化规则编辑表单
```

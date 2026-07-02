# API Mock Content-Type Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve API Mock route editing so users can pick common `Content-Type` values, enter custom values, and get targeted validation or warnings for mismatched response content.

**Architecture:** Keep persistence and Rust response behavior unchanged. Add focused pure functions in `apps/desktop/src/utils/apiMock.ts`, cover them with Vitest, then keep `ApiMockPanel.vue` limited to UI binding, message display, and save/import orchestration.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Element Plus, Vitest, existing Tauri bridge.

## Global Constraints

- Do not modify SQLite schema.
- Do not modify `apps/desktop/src/types/api-mock.ts`; `contentType` remains `string`.
- Do not modify `apps/desktop/src-tauri/src/tools/api_mock.rs`.
- Do not implement Body templates, auto-formatting, or generated response examples.
- Do not restrict file upload types.
- Empty `Content-Type` remains allowed and falls back to existing backend/file behavior.
- Block only clear errors: invalid JSON for `application/json`, CR/LF in `Content-Type`, and malformed non-empty `Content-Type`.
- File content-type mismatch is a warning only; `application/octet-stream` is treated as generic binary and does not warn.

---

## File Structure

- `apps/desktop/src/utils/apiMock.ts`
  - Add common content-type preset data.
  - Add content-type trimming, MIME normalization, header validation, static body validation, and file mismatch warning helpers.
  - Keep existing file inference and runtime helpers in place.
- `apps/desktop/src/utils/apiMock.test.ts`
  - Add focused unit tests for presets, normalization, validation, warning behavior, and custom MIME allowance.
- `apps/desktop/src/components/ApiMockPanel.vue`
  - Replace the `Content-Type` input with a filterable, creatable, clearable `el-select`.
  - Use utility helpers before route save and after file import.
  - Show `ElMessage.error` for blocking errors and `ElMessage.warning` for non-blocking warnings.

---

### Task 1: Content-Type Utility Functions

**Files:**
- Modify: `apps/desktop/src/utils/apiMock.test.ts`
- Modify: `apps/desktop/src/utils/apiMock.ts`

**Interfaces:**
- Consumes: existing `inferMockContentTypeFromFileName(fileName: string): string`
- Produces:
  - `API_MOCK_CONTENT_TYPE_PRESETS: ApiMockContentTypePreset[]`
  - `trimMockContentType(contentType: string): string`
  - `normalizeMockContentType(contentType: string): string`
  - `validateMockContentTypeHeader(contentType: string): ApiMockValidationResult`
  - `validateMockStaticResponseContent(input: { contentType: string; bodyText: string }): ApiMockContentValidationNotice | null`
  - `getMockFileContentTypeWarning(input: { contentType: string; fileName: string }): string`

- [ ] **Step 1: Add failing utility imports and tests**

Modify the import list in `apps/desktop/src/utils/apiMock.test.ts` so it imports the new helpers:

```ts
import {
  API_MOCK_CONTENT_TYPE_PRESETS,
  buildMockRouteSummary,
  deriveMockProjectRuntimeState,
  formatMockFileSize,
  getMockFileContentTypeWarning,
  getMockProjectAccessUrl,
  getMockProjectRuntimeAction,
  getMockRouteSpecificityLabel,
  isMockProjectRestartRequired,
  normalizeMockContentType,
  normalizeMockHeaderRows,
  resolveMockFileContentType,
  trimMockContentType,
  validateMockContentTypeHeader,
  validateMockCorsConfig,
  validateMockPathPattern,
  validateMockStaticResponseContent,
} from "./apiMock";
```

Append these tests inside the existing `describe("apiMock utils", () => { ... })` block:

```ts
  it("exposes common content type presets", () => {
    const values = API_MOCK_CONTENT_TYPE_PRESETS.map((item) => item.value);

    expect(values).toEqual(
      expect.arrayContaining([
        "application/json; charset=utf-8",
        "application/json",
        "text/plain; charset=utf-8",
        "text/html; charset=utf-8",
        "application/xml",
        "text/xml; charset=utf-8",
        "text/csv; charset=utf-8",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
        "image/png",
        "image/jpeg",
        "image/svg+xml",
        "image/webp",
        "image/gif",
        "application/pdf",
        "application/zip",
        "application/wasm",
        "application/octet-stream",
        "text/css; charset=utf-8",
        "text/javascript; charset=utf-8",
      ]),
    );
    expect(new Set(values).size).toBe(values.length);
  });

  it("normalizes content type MIME without parameters", () => {
    expect(normalizeMockContentType(" Application/JSON; Charset=UTF-8 ")).toBe("application/json");
    expect(normalizeMockContentType("application/vnd.lazycat.mock+json; version=1")).toBe(
      "application/vnd.lazycat.mock+json",
    );
    expect(normalizeMockContentType("")).toBe("");
  });

  it("trims content type before saving", () => {
    expect(trimMockContentType("  application/json; charset=utf-8  ")).toBe("application/json; charset=utf-8");
  });

  it("rejects unsafe or malformed content type values", () => {
    expect(validateMockContentTypeHeader("").ok).toBe(true);
    expect(validateMockContentTypeHeader(" application/vnd.lazycat.mock+json; version=1 ").ok).toBe(true);
    expect(validateMockContentTypeHeader("application/json\r\nX-Bad: 1")).toEqual({
      ok: false,
      message: "Content-Type 不能包含换行符",
    });
    expect(validateMockContentTypeHeader("json")).toEqual({
      ok: false,
      message: "Content-Type 必须是 type/subtype 格式",
    });
  });

  it("blocks invalid JSON when the response content type is JSON", () => {
    expect(
      validateMockStaticResponseContent({
        contentType: "application/json; charset=utf-8",
        bodyText: "{ bad json",
      }),
    ).toEqual({
      level: "error",
      message: "当前 Content-Type 是 JSON，但响应 Body 不是合法 JSON",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "application/json",
        bodyText: "{ \"ok\": true }",
      }),
    ).toBeNull();
  });

  it("warns for response content types that need user confirmation", () => {
    expect(
      validateMockStaticResponseContent({
        contentType: "application/xml",
        bodyText: "<root>",
      }),
    ).toEqual({
      level: "warning",
      message: "当前 Content-Type 是 XML，请确认响应 Body 是正确的 XML 内容",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "text/html; charset=utf-8",
        bodyText: "<main>",
      }),
    ).toEqual({
      level: "warning",
      message: "当前 Content-Type 是 HTML，请确认响应 Body 是 HTML 内容",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "multipart/form-data",
        bodyText: "",
      }),
    ).toEqual({
      level: "warning",
      message: "multipart/form-data 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    });
  });

  it("warns when selected content type and imported file extension disagree", () => {
    expect(getMockFileContentTypeWarning({ contentType: "application/pdf", fileName: "avatar.png" })).toBe(
      "上传文件看起来是 image/png，当前 Content-Type 是 application/pdf，请确认是否正确。",
    );
    expect(
      getMockFileContentTypeWarning({ contentType: "text/plain; charset=utf-8", fileName: "readme.txt" }),
    ).toBe("");
    expect(getMockFileContentTypeWarning({ contentType: "application/octet-stream", fileName: "avatar.png" })).toBe(
      "",
    );
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
pnpm test src/utils/apiMock.test.ts
```

Expected: FAIL with TypeScript/Vitest errors that the new exports do not exist.

- [ ] **Step 3: Add utility implementation**

In `apps/desktop/src/utils/apiMock.ts`, add these exports after `DEFAULT_API_MOCK_CONTENT_TYPE`:

```ts
export interface ApiMockContentTypePreset {
  label: string;
  value: string;
}

export type ApiMockContentValidationNotice = {
  level: "error" | "warning";
  message: string;
};

export const API_MOCK_CONTENT_TYPE_PRESETS: ApiMockContentTypePreset[] = [
  { label: "JSON UTF-8", value: "application/json; charset=utf-8" },
  { label: "JSON", value: "application/json" },
  { label: "Plain Text", value: "text/plain; charset=utf-8" },
  { label: "HTML", value: "text/html; charset=utf-8" },
  { label: "XML", value: "application/xml" },
  { label: "XML Text", value: "text/xml; charset=utf-8" },
  { label: "CSV", value: "text/csv; charset=utf-8" },
  { label: "Form URL Encoded", value: "application/x-www-form-urlencoded" },
  { label: "Multipart Form", value: "multipart/form-data" },
  { label: "PNG", value: "image/png" },
  { label: "JPEG", value: "image/jpeg" },
  { label: "SVG", value: "image/svg+xml" },
  { label: "WebP", value: "image/webp" },
  { label: "GIF", value: "image/gif" },
  { label: "PDF", value: "application/pdf" },
  { label: "ZIP", value: "application/zip" },
  { label: "WASM", value: "application/wasm" },
  { label: "Binary", value: "application/octet-stream" },
  { label: "CSS", value: "text/css; charset=utf-8" },
  { label: "JavaScript", value: "text/javascript; charset=utf-8" },
];
```

Add these functions before `resolveMockFileContentType`:

```ts
export function trimMockContentType(contentType: string): string {
  return contentType.trim();
}

export function normalizeMockContentType(contentType: string): string {
  return trimMockContentType(contentType).split(";")[0]?.trim().toLowerCase() ?? "";
}

export function validateMockContentTypeHeader(contentType: string): ApiMockValidationResult {
  const value = trimMockContentType(contentType);
  if (!value) return ok();
  if (/[\r\n]/.test(value)) return fail("Content-Type 不能包含换行符");

  const mime = value.split(";")[0]?.trim() ?? "";
  if (!/^[A-Za-z0-9!#$&^_.+-]+\/[A-Za-z0-9!#$&^_.+-]+$/.test(mime)) {
    return fail("Content-Type 必须是 type/subtype 格式");
  }

  return ok();
}

export function validateMockStaticResponseContent(input: {
  contentType: string;
  bodyText: string;
}): ApiMockContentValidationNotice | null {
  const mime = normalizeMockContentType(input.contentType);
  if (!mime) return null;

  if (mime === "application/json") {
    try {
      JSON.parse(input.bodyText);
      return null;
    } catch {
      return {
        level: "error",
        message: "当前 Content-Type 是 JSON，但响应 Body 不是合法 JSON",
      };
    }
  }

  if (mime === "application/xml" || mime === "text/xml") {
    return {
      level: "warning",
      message: "当前 Content-Type 是 XML，请确认响应 Body 是正确的 XML 内容",
    };
  }
  if (mime === "text/html") {
    return {
      level: "warning",
      message: "当前 Content-Type 是 HTML，请确认响应 Body 是 HTML 内容",
    };
  }
  if (mime === "application/x-www-form-urlencoded") {
    return {
      level: "warning",
      message: "application/x-www-form-urlencoded 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    };
  }
  if (mime === "multipart/form-data") {
    return {
      level: "warning",
      message: "multipart/form-data 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    };
  }

  return null;
}

export function getMockFileContentTypeWarning(input: { contentType: string; fileName: string }): string {
  const current = normalizeMockContentType(input.contentType);
  const inferred = normalizeMockContentType(inferMockContentTypeFromFileName(input.fileName));
  if (!current || !inferred || current === "application/octet-stream" || current === inferred) return "";
  return `上传文件看起来是 ${inferred}，当前 Content-Type 是 ${current}，请确认是否正确。`;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
pnpm test src/utils/apiMock.test.ts
```

Expected: PASS for `apiMock.test.ts`.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/utils/apiMock.ts apps/desktop/src/utils/apiMock.test.ts
git commit -m "feat(api-mock): 添加 content-type 校验工具"
```

---

### Task 2: Save-Time Validation Wiring

**Files:**
- Modify: `apps/desktop/src/components/ApiMockPanel.vue`

**Interfaces:**
- Consumes:
  - `trimMockContentType(contentType: string): string`
  - `validateMockContentTypeHeader(contentType: string): ApiMockValidationResult`
  - `validateMockStaticResponseContent(input): ApiMockContentValidationNotice | null`
  - `getMockFileContentTypeWarning(input): string`
- Produces: route save payload uses the trimmed `contentType` and save blocks only on validation errors.

- [ ] **Step 1: Import save-time helpers**

In `apps/desktop/src/components/ApiMockPanel.vue`, update the existing `../utils/apiMock` import block to include:

```ts
  getMockFileContentTypeWarning,
  trimMockContentType,
  validateMockContentTypeHeader,
  validateMockStaticResponseContent,
```

The import block should contain these names with the existing imports:

```ts
import {
  API_MOCK_METHODS,
  DEFAULT_API_MOCK_CONTENT_TYPE,
  DEFAULT_API_MOCK_CORS,
  deriveMockProjectRuntimeState,
  formatMockFileSize,
  getMockFileContentTypeWarning,
  getMockProjectAccessUrl,
  getMockProjectRuntimeAction,
  getMockRouteSpecificityLabel,
  normalizeMockHeaderRows,
  resolveMockFileContentType,
  trimMockContentType,
  validateMockContentTypeHeader,
  validateMockCorsConfig,
  validateMockPathPattern,
  validateMockStaticResponseContent,
} from "../utils/apiMock";
```

- [ ] **Step 2: Add validation before route save payload**

In `saveRoute()`, after the existing CORS validation block and before the file-required check, insert:

```ts
  const contentType = trimMockContentType(routeForm.contentType);
  const contentTypeResult = validateMockContentTypeHeader(contentType);
  if (!contentTypeResult.ok) {
    ElMessage.error(contentTypeResult.message);
    return;
  }

  const staticContentNotice =
    routeForm.responseKind === "static_body"
      ? validateMockStaticResponseContent({ contentType, bodyText: routeForm.bodyText })
      : null;
  if (staticContentNotice?.level === "error") {
    ElMessage.error(staticContentNotice.message);
    return;
  }
  if (staticContentNotice?.level === "warning") {
    ElMessage.warning(staticContentNotice.message);
  }

  if (routeForm.responseKind === "file" && routeFile.value) {
    const fileContentTypeWarning = getMockFileContentTypeWarning({
      contentType,
      fileName: routeFile.value.originalName,
    });
    if (fileContentTypeWarning) {
      ElMessage.warning(fileContentTypeWarning);
    }
  }
  routeForm.contentType = contentType;
```

Then change the save payload field from:

```ts
      contentType: routeForm.contentType,
```

to:

```ts
      contentType,
```

- [ ] **Step 3: Run focused test and typecheck**

Run:

```bash
pnpm test src/utils/apiMock.test.ts
pnpm typecheck
```

Expected: tests pass and typecheck succeeds.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/ApiMockPanel.vue
git commit -m "feat(api-mock): 保存路由时校验 content-type"
```

---

### Task 3: Content-Type Select UI and File Import Warning

**Files:**
- Modify: `apps/desktop/src/components/ApiMockPanel.vue`

**Interfaces:**
- Consumes:
  - `API_MOCK_CONTENT_TYPE_PRESETS`
  - `getMockFileContentTypeWarning(input): string`
  - `resolveMockFileContentType(currentContentType: string, fileName: string): string`
- Produces: route form uses a filterable, creatable, clearable select; file import warns on selected type versus inferred file type mismatch.

- [ ] **Step 1: Import preset list**

Add `API_MOCK_CONTENT_TYPE_PRESETS` to the `../utils/apiMock` import block:

```ts
  API_MOCK_CONTENT_TYPE_PRESETS,
```

- [ ] **Step 2: Replace the Content-Type input**

Replace this template block:

```vue
              <el-form-item label="Content-Type">
                <el-input v-model="routeForm.contentType" placeholder="application/json; charset=utf-8" />
              </el-form-item>
```

with:

```vue
              <el-form-item label="Content-Type">
                <el-select
                  v-model="routeForm.contentType"
                  filterable
                  allow-create
                  clearable
                  default-first-option
                  placeholder="application/json; charset=utf-8"
                >
                  <el-option
                    v-for="preset in API_MOCK_CONTENT_TYPE_PRESETS"
                    :key="preset.value"
                    :label="preset.value"
                    :value="preset.value"
                  >
                    <div class="content-type-option">
                      <span>{{ preset.label }}</span>
                      <small>{{ preset.value }}</small>
                    </div>
                  </el-option>
                </el-select>
              </el-form-item>
```

- [ ] **Step 3: Warn after importing mismatched files**

In `pickFile()`, after `routeForm.contentType = contentType;` and `routeFile.value = result.file;`, add:

```ts
    const fileContentTypeWarning = getMockFileContentTypeWarning({
      contentType,
      fileName: selected,
    });
    if (fileContentTypeWarning) {
      ElMessage.warning(fileContentTypeWarning);
    }
```

The resulting success branch should be:

```ts
    routeForm.contentType = contentType;
    routeFile.value = result.file;
    const fileContentTypeWarning = getMockFileContentTypeWarning({
      contentType,
      fileName: selected,
    });
    if (fileContentTypeWarning) {
      ElMessage.warning(fileContentTypeWarning);
    }
```

- [ ] **Step 4: Add compact option styling**

In the scoped style block, add this near the other form styles:

```css
.content-type-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.content-type-option span,
.content-type-option small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.content-type-option span {
  font-weight: 600;
  color: #172033;
}

.content-type-option small {
  color: #64748b;
}
```

- [ ] **Step 5: Run focused test and typecheck**

Run:

```bash
pnpm test src/utils/apiMock.test.ts
pnpm typecheck
```

Expected: tests pass and typecheck succeeds.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/ApiMockPanel.vue
git commit -m "feat(api-mock): 支持选择常见 content-type"
```

---

### Task 4: Final Verification

**Files:**
- No code changes expected.
- Optionally modify `process.md` only if implementation reveals a reusable project lesson not already covered by the existing API Mock entries.

**Interfaces:**
- Consumes: completed Tasks 1-3.
- Produces: verified implementation ready for user review.

- [ ] **Step 1: Run targeted tests**

Run:

```bash
pnpm test src/utils/apiMock.test.ts
```

Expected: PASS.

- [ ] **Step 2: Run typecheck**

Run:

```bash
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 3: Run renderer build**

Run:

```bash
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS.

- [ ] **Step 4: Inspect diff**

Run:

```bash
git diff --stat
git diff
```

Expected: only these files changed:

```text
apps/desktop/src/components/ApiMockPanel.vue
apps/desktop/src/utils/apiMock.ts
apps/desktop/src/utils/apiMock.test.ts
```

- [ ] **Step 5: Final commit if previous tasks were not committed individually**

If Task 1-3 commits were skipped during inline execution, run:

```bash
git add apps/desktop/src/components/ApiMockPanel.vue apps/desktop/src/utils/apiMock.ts apps/desktop/src/utils/apiMock.test.ts
git commit -m "feat(api-mock): 优化 content-type 选择与校验"
```

Expected: one feature commit containing the implementation.

---

## Self-Review

- Spec coverage:
  - Common preset selection: Task 1 preset data, Task 3 select UI.
  - Custom `Content-Type`: Task 3 `allow-create`, Task 1 validation allows vendor/private MIME values.
  - JSON blocking validation: Task 1 tests and helper, Task 2 save wiring.
  - XML/HTML/Form/Multipart warnings: Task 1 helper/tests, Task 2 save wiring.
  - File mismatch warning: Task 1 helper/tests, Task 2 save-time check, Task 3 import-time check.
  - Persistence/backend unchanged: Global Constraints and file structure keep changes in frontend component and utils only.
- Placeholder scan: no `TBD`, `TODO`, `implement later`, or unspecified validation steps.
- Type consistency:
  - `ApiMockContentValidationNotice` is produced in Task 1 and consumed in Task 2.
  - `getMockFileContentTypeWarning` signature is identical in Tasks 1-3.
  - `trimMockContentType` and `normalizeMockContentType` names are consistent across tests, implementation, and component wiring.

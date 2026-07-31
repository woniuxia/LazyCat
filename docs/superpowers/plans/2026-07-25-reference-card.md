# 置顶参考卡实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付可由全局快捷键或 Spotlight 创建的多张独立置顶参考卡，复用本地 Monaco 编辑能力，并确保内容只存在当前应用会话。

**Architecture:** Rust `reference_card` 模块统一读取/接收文本、来源判重、窗口注册和动态 Tauri 窗口生命周期；Spotlight 与全局快捷键只提供入口。每张卡片加载独立 Vue 入口并复用 `MonacoPane`，正文留在渲染进程内存，Rust 仅持有来源指纹、窗口关系和 ready 前的初始化文本。

**Tech Stack:** Tauri 2、Rust、windows-sys、Vue 3、TypeScript、Monaco Editor、Vitest、pnpm。

---

## 文件职责与落点

**新增文件**

- `apps/desktop/src/utils/monacoLanguages.ts`：唯一的 Monaco 语言选项、扩展名和剪贴板类型映射。
- `apps/desktop/src/utils/monacoLanguages.test.ts`：语言映射和参考卡文本边界测试。
- `apps/desktop/src/spotlight/clipboard-suggestions.ts`：把当前剪贴板文本转换为有序 Spotlight 建议项。
- `apps/desktop/src/spotlight/clipboard-suggestions.test.ts`：智能工具建议、参考卡建议和搜索字段测试。
- `apps/desktop/src/spotlight/providers/suggestion.test.ts`：判别式 suggestion action 的执行路由测试。
- `apps/desktop/src-tauri/src/clipboard.rs`：共享 Windows Unicode 剪贴板读取、RAII 关闭和有限重试。
- `apps/desktop/src-tauri/src/reference_card/state.rs`：来源指纹、卡片上限、待初始化文本和最近使用顺序。
- `apps/desktop/src-tauri/src/reference_card/position.rs`：多显示器工作区内的错位定位纯函数。
- `apps/desktop/src-tauri/src/reference_card/mod.rs`：统一创建/聚焦、窗口构建、ready 握手、命令和错误通知。
- `apps/desktop/src/types/reference-card.ts`：参考卡初始化事件的前端契约。
- `apps/desktop/src/ReferenceCardApp.ts`：独立参考卡 Vue 挂载入口。
- `apps/desktop/src/components/ReferenceCard.vue`：标题栏、Monaco、复制、语言和关闭交互。
- `apps/desktop/src/components/ReferenceCard.contract.test.ts`：卡片入口、窗口接线、快捷键和 capability 的源契约测试。

**主要修改文件**

- `apps/desktop/src-tauri/src/tools/inbox.rs`：复用共享剪贴板 guard 和 Unicode 文本读取，不保留第二份低层实现。
- `apps/desktop/src-tauri/src/{main.rs,events.rs}`：模块注册、动态窗口标题、命名快捷键、command、关闭清理和初始化事件。
- `apps/desktop/src-tauri/capabilities/default.json`：允许 `reference-card-*` 动态窗口。
- `apps/desktop/src/{main.ts,bridge/tauri.ts,bridge/events.ts}`：挂载分流、show/ready bridge 和事件常量。
- `apps/desktop/src/components/{MonacoPane.vue,SnippetPanel.vue,SpotlightPanel.vue,SettingsPanel.vue}`：显式 word-wrap/focus API、共享语言列表、双剪贴板建议和快捷键表单。
- `apps/desktop/src/spotlight/providers/suggestion.ts`：按 action kind 直接创建参考卡或继续打开工具。
- `apps/desktop/src/App.vue`：默认快捷键加载、设置 props 和启动注册。
- `docs/experience/{architecture.md,spotlight-and-launcher.md,vault-and-inbox.md}`：沉淀动态窗口、Spotlight 判别式建议和回采边界。

## 统一契约

实现时以下命名和边界不可漂移：

```ts
export type ClipboardSuggestionAction =
  | { kind: "open-tool"; toolId: string; text: string }
  | { kind: "open-reference-card"; text: string };

export interface ReferenceCardInitPayload {
  content: string;
}
```

```rust
pub(crate) const REFERENCE_CARD_PREFIX: &str = "reference-card-";
pub(crate) const REFERENCE_CARD_TITLE: &str = "置顶参考";
pub(crate) const MAX_CARDS: usize = 6;
pub(crate) const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
```

来源指纹基于 `CRLF -> LF` 后的 `trim()` 文本计算，但卡片收到的正文必须保持原样。用户编辑正文后不重算来源指纹。

---

### Task 1: 提取 Monaco 语言目录并实现参考卡语言识别

**Files:**

- Create: `apps/desktop/src/utils/monacoLanguages.ts`
- Create: `apps/desktop/src/utils/monacoLanguages.test.ts`
- Modify: `apps/desktop/src/components/SnippetPanel.vue:237-271,301-306,513`

- [ ] **Step 1: 写失败测试**

创建 `monacoLanguages.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import {
  MAX_REFERENCE_CARD_TEXT_BYTES,
  MONACO_LANGUAGE_EXTENSIONS,
  MONACO_LANGUAGE_OPTIONS,
  detectClipboardMonacoLanguage,
  validateReferenceCardText,
} from "./monacoLanguages";

describe("Monaco language catalog", () => {
  it("keeps the shared language list and snippet extensions", () => {
    expect(MONACO_LANGUAGE_OPTIONS).toContain("plaintext");
    expect(MONACO_LANGUAGE_OPTIONS).toContain("typescript");
    expect(MONACO_LANGUAGE_EXTENSIONS.typescript).toBe("ts");
    expect(MONACO_LANGUAGE_EXTENSIONS.plaintext).toBe("txt");
  });

  it.each([
    ['{"port":8080}', "json"],
    ["<html><body>demo</body></html>", "html"],
    ["SELECT * FROM users WHERE id = 1", "sql"],
    ["public class Demo { private int id; }", "java"],
    ["普通临时参考文字", "plaintext"],
  ])("maps clipboard content %s to %s", (text, language) => {
    expect(detectClipboardMonacoLanguage(text)).toBe(language);
  });

  it("accepts non-empty text up to 8 MiB and rejects invalid input", () => {
    expect(validateReferenceCardText("  demo  ")).toEqual({ ok: true });
    expect(validateReferenceCardText(" \r\n ")).toEqual({
      ok: false,
      message: "剪贴板中没有可用文本",
    });
    expect(validateReferenceCardText("a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES + 1))).toEqual({
      ok: false,
      message: "参考文本不能超过 8 MiB",
    });
  });
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/monacoLanguages.test.ts`

Expected: FAIL，提示 `monacoLanguages.ts` 不存在。

- [ ] **Step 3: 实现共享目录和纯函数**

创建 `monacoLanguages.ts`：

```ts
import { detectClipboardContent, type ClipboardContentType } from "./clipboard-detect";

export const MAX_REFERENCE_CARD_TEXT_BYTES = 8 * 1024 * 1024;

export const MONACO_LANGUAGE_OPTIONS = [
  "javascript",
  "typescript",
  "python",
  "java",
  "go",
  "rust",
  "sql",
  "html",
  "css",
  "json",
  "xml",
  "yaml",
  "bash",
  "shell",
  "markdown",
  "plaintext",
  "c",
  "cpp",
  "csharp",
  "php",
  "ruby",
  "swift",
  "kotlin",
  "scala",
  "lua",
  "r",
  "dart",
  "dockerfile",
  "graphql",
  "toml",
] as const;

export const MONACO_LANGUAGE_EXTENSIONS: Record<string, string> = {
  javascript: "js",
  typescript: "ts",
  python: "py",
  java: "java",
  go: "go",
  rust: "rs",
  sql: "sql",
  html: "html",
  css: "css",
  json: "json",
  xml: "xml",
  yaml: "yml",
  bash: "sh",
  shell: "sh",
  markdown: "md",
  plaintext: "txt",
  c: "c",
  cpp: "cpp",
  csharp: "cs",
  php: "php",
  ruby: "rb",
  swift: "swift",
  kotlin: "kt",
  scala: "scala",
  lua: "lua",
  r: "r",
  dart: "dart",
  dockerfile: "dockerfile",
  graphql: "graphql",
  toml: "toml",
};

const CLIPBOARD_LANGUAGE_MAP: Partial<Record<ClipboardContentType, string>> = {
  json: "json",
  xml: "xml",
  html: "html",
  sql: "sql",
  java: "java",
};

export function detectClipboardMonacoLanguage(text: string): string {
  const type = detectClipboardContent(text)?.type;
  return (type && CLIPBOARD_LANGUAGE_MAP[type]) || "plaintext";
}

export function validateReferenceCardText(
  text: string,
): { ok: true } | { ok: false; message: string } {
  if (!text.trim()) return { ok: false, message: "剪贴板中没有可用文本" };
  if (new TextEncoder().encode(text).byteLength > MAX_REFERENCE_CARD_TEXT_BYTES) {
    return { ok: false, message: "参考文本不能超过 8 MiB" };
  }
  return { ok: true };
}
```

在 `SnippetPanel.vue` 删除本地 `defaultLanguages` 和 `languageExtensionMap`，改为：

```ts
import { MONACO_LANGUAGE_EXTENSIONS, MONACO_LANGUAGE_OPTIONS } from "../utils/monacoLanguages";

const languageOptions = computed(() => {
  const used = new Set(current.value?.fragments.map((fragment) => fragment.language) ?? []);
  return [
    ...Array.from(used),
    ...MONACO_LANGUAGE_OPTIONS.filter((language) => !used.has(language)),
  ];
});
```

导出文件名处使用：

```ts
const ext =
  MONACO_LANGUAGE_EXTENSIONS[fragment.language.toLowerCase()] ?? fragment.language.toLowerCase();
```

- [ ] **Step 4: 运行测试和类型检查确认 GREEN**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/monacoLanguages.test.ts`

Expected: PASS，5 组语言映射、语言目录和 8 MiB 边界通过。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS，Snippet 语言类型和导出扩展名无回归。

- [ ] **Step 5: 提交共享语言能力**

```powershell
git add apps/desktop/src/utils/monacoLanguages.ts apps/desktop/src/utils/monacoLanguages.test.ts apps/desktop/src/components/SnippetPanel.vue
git commit -m "refactor: 共享 Monaco 语言目录"
```

---

### Task 2: 将 Spotlight 剪贴板建议改为判别式双结果

**Files:**

- Create: `apps/desktop/src/spotlight/clipboard-suggestions.ts`
- Create: `apps/desktop/src/spotlight/clipboard-suggestions.test.ts`
- Create: `apps/desktop/src/spotlight/providers/suggestion.test.ts`
- Modify: `apps/desktop/src/spotlight/providers/suggestion.ts`
- Modify: `apps/desktop/src/components/SpotlightPanel.vue:188-225,430-475,914-984`
- Modify: `apps/desktop/src/bridge/tauri.ts`

- [ ] **Step 1: 写建议构建和执行路由失败测试**

`clipboard-suggestions.test.ts` 固定顺序和检索字段：

```ts
import { describe, expect, it } from "vitest";
import { buildClipboardSuggestionItems } from "./clipboard-suggestions";

describe("buildClipboardSuggestionItems", () => {
  it("keeps the specialized action first and appends reference card", () => {
    const items = buildClipboardSuggestionItems('{"port":8080}');
    expect(items.map((item) => item.payload?.suggestionAction)).toEqual([
      { kind: "open-tool", toolId: "formatter", text: '{"port":8080}' },
      { kind: "open-reference-card", text: '{"port":8080}' },
    ]);
    expect(items[0].weight).toBeGreaterThan(items[1].weight ?? 0);
  });

  it("offers only reference card for unknown text", () => {
    const items = buildClipboardSuggestionItems("临时对照内容");
    expect(items).toHaveLength(1);
    expect(items[0].payload?.suggestionAction).toEqual({
      kind: "open-reference-card",
      text: "临时对照内容",
    });
  });

  it("makes the reference result searchable", () => {
    const [item] = buildClipboardSuggestionItems("临时对照内容");
    const text = item.searchFields.map((field) => field.text);
    expect(text).toEqual(
      expect.arrayContaining(["置顶参考卡", "参考", "置顶", "卡片", "clipboard", "reference"]),
    );
  });

  it("returns no result for empty or oversized text", () => {
    expect(buildClipboardSuggestionItems(" \n ")).toEqual([]);
    expect(buildClipboardSuggestionItems("a".repeat(8 * 1024 * 1024 + 1))).toEqual([]);
  });
});
```

`suggestion.test.ts` mock `showReferenceCard` 和 Tauri `invoke`：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke, showReferenceCard } = vi.hoisted(() => ({
  invoke: vi.fn(),
  showReferenceCard: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../../bridge/tauri", () => ({ showReferenceCard }));

import { suggestionProvider } from "./suggestion";

const item = (suggestionAction: Record<string, unknown>) => ({
  providerId: "suggestion" as const,
  itemId: "test",
  title: "test",
  searchFields: [],
  payload: { suggestionAction },
});

beforeEach(() => {
  invoke.mockReset();
  showReferenceCard.mockReset();
});

describe("suggestionProvider", () => {
  it("creates a reference card without opening the main window", async () => {
    showReferenceCard.mockResolvedValue({ outcome: "created", windowLabel: "reference-card-1" });
    const result = await suggestionProvider.defaultAction(
      item({ kind: "open-reference-card", text: "demo" }),
      {} as never,
    );
    expect(showReferenceCard).toHaveBeenCalledWith("demo");
    expect(invoke).not.toHaveBeenCalledWith("spotlight_pick", expect.anything());
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("keeps the existing tool route", async () => {
    const result = await suggestionProvider.defaultAction(
      item({ kind: "open-tool", toolId: "formatter", text: "demo" }),
      {} as never,
    );
    expect(invoke).toHaveBeenCalledWith("spotlight_pick", {
      target: "formatter",
      text: "demo",
      source: "clipboard-suggestion",
    });
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("rejects malformed payloads explicitly", async () => {
    const result = await suggestionProvider.defaultAction(item({ kind: "unknown" }), {} as never);
    expect(result.errorMessage).toContain("无效");
  });
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `pnpm --filter @lazycat/desktop test -- src/spotlight/clipboard-suggestions.test.ts src/spotlight/providers/suggestion.test.ts`

Expected: FAIL，建议构建模块和 `showReferenceCard` bridge 尚不存在。

- [ ] **Step 3: 实现建议构建器**

`clipboard-suggestions.ts`：

```ts
import { isRealToolId } from "../composables/toolCatalog";
import { detectClipboardContent } from "../utils/clipboard-detect";
import { toPinyinInitials } from "../utils/fuzzy-match";
import { validateReferenceCardText } from "../utils/monacoLanguages";
import type { SpotlightItem } from "./types";

export type ClipboardSuggestionAction =
  | { kind: "open-tool"; toolId: string; text: string }
  | { kind: "open-reference-card"; text: string };

function field(text: string, weight = 1) {
  return { text, initials: toPinyinInitials(text), weight };
}

function preview(text: string): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > 32 ? `${oneLine.slice(0, 32)}…` : oneLine;
}

export function buildClipboardSuggestionItems(text: string): SpotlightItem[] {
  if (!validateReferenceCardText(text).ok) return [];
  const items: SpotlightItem[] = [];
  const detected = detectClipboardContent(text);
  const toolAction = detected?.actions.find(
    (action) => action.kind === "tool" && isRealToolId(action.toolId),
  );
  if (toolAction?.kind === "tool") {
    items.push({
      providerId: "suggestion",
      itemId: `suggestion:tool:${toolAction.toolId}`,
      title: `${toolAction.toolName}（剪贴板：${preview(text)}）`,
      subtitle: "Enter 打开并预填剪贴板内容",
      badge: { short: "建议", tone: "warn" },
      searchFields: [],
      weight: 2,
      payload: {
        suggestionAction: { kind: "open-tool", toolId: toolAction.toolId, text },
      },
    });
  }
  items.push({
    providerId: "suggestion",
    itemId: "suggestion:reference-card",
    title: `创建置顶参考卡（剪贴板：${preview(text)}）`,
    subtitle: detected ? `${detected.label} · Enter 创建或聚焦参考卡` : "Enter 创建或聚焦参考卡",
    badge: { short: "参考", tone: "primary" },
    searchFields: ["置顶参考卡", "参考", "置顶", "卡片", "clipboard", "reference"].map((value) =>
      field(value),
    ),
    weight: 1.5,
    payload: { suggestionAction: { kind: "open-reference-card", text } },
  });
  return items;
}
```

- [ ] **Step 4: 接入 provider、bridge 和 Spotlight 结果集合**

在 `bridge/tauri.ts` 增加：

```ts
export interface ReferenceCardShowResult {
  outcome: "created" | "focused";
  windowLabel: string;
}

export async function showReferenceCard(text: string): Promise<ReferenceCardShowResult> {
  return invoke<ReferenceCardShowResult>("reference_card_show", { text });
}
```

`suggestion.ts` 的 defaultAction 使用判别字段：

```ts
import { showReferenceCard } from "../../bridge/tauri";
import type { ClipboardSuggestionAction } from "../clipboard-suggestions";

const action = item.payload?.suggestionAction as ClipboardSuggestionAction | undefined;
if (action?.kind === "open-reference-card" && typeof action.text === "string") {
  await showReferenceCard(action.text);
  return { closeSpotlight: true };
}
if (
  action?.kind === "open-tool" &&
  typeof action.toolId === "string" &&
  typeof action.text === "string"
) {
  await invoke("spotlight_pick", {
    target: action.toolId,
    text: action.text,
    source: "clipboard-suggestion",
  });
  return { closeSpotlight: true };
}
return { errorMessage: "无效的剪贴板建议" };
```

`SpotlightPanel.vue` 把单项状态改为：

```ts
const clipboardSuggestionItems = ref<SpotlightItem[]>([]);

async function refreshClipboardSuggestions() {
  await initSettings().catch(() => undefined);
  if (getSetting("clipboard_detection") === "false") {
    clipboardSuggestionItems.value = [];
    return;
  }
  try {
    clipboardSuggestionItems.value = buildClipboardSuggestionItems(
      await navigator.clipboard.readText(),
    );
  } catch {
    clipboardSuggestionItems.value = [];
  }
}
```

将动态项放入统一 provider map，删除当前空查询中的手工单项插入分支：

```ts
const searchableItemsByProvider = computed(() => {
  const merged = mergeSpotlightProviderItems(itemsByProvider.value, queryItemsByProvider.value);
  const next = new Map(merged);
  next.set("suggestion", clipboardSuggestionItems.value);
  return next;
});
```

把所有 `refreshClipboardSuggestion()` 调用改为 `refreshClipboardSuggestions()`。空查询现有 provider 排序会按 weight 保持智能工具项在前；非空查询由参考项的 `searchFields` 正常命中。

- [ ] **Step 5: 运行定向测试和类型检查确认 GREEN**

Run: `pnpm --filter @lazycat/desktop test -- src/spotlight/clipboard-suggestions.test.ts src/spotlight/providers/suggestion.test.ts src/spotlight/search.test.ts`

Expected: PASS，智能建议顺序、未知文本、关键词检索和 provider 执行通过。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS，Spotlight item payload 和 bridge 返回类型一致。

- [ ] **Step 6: 提交 Spotlight 双入口准备**

```powershell
git add apps/desktop/src/spotlight/clipboard-suggestions.ts apps/desktop/src/spotlight/clipboard-suggestions.test.ts apps/desktop/src/spotlight/providers/suggestion.ts apps/desktop/src/spotlight/providers/suggestion.test.ts apps/desktop/src/components/SpotlightPanel.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(spotlight): 添加置顶参考卡建议"
```

---

### Task 3: 建立共享剪贴板读取、卡片会话状态和定位纯函数

**Files:**

- Create: `apps/desktop/src-tauri/src/clipboard.rs`
- Create: `apps/desktop/src-tauri/src/reference_card/state.rs`
- Create: `apps/desktop/src-tauri/src/reference_card/position.rs`
- Create: `apps/desktop/src-tauri/src/reference_card/mod.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs:3-5`
- Modify: `apps/desktop/src-tauri/src/tools/inbox.rs:1530-1579,1879-1898,1984`

- [ ] **Step 1: 写共享剪贴板、注册表和定位失败测试**

`clipboard.rs` 固定有限重试：

```rust
#[cfg(test)]
mod tests {
    use super::retry_read;

    #[test]
    fn retry_read_stops_after_success() {
        let mut calls = 0;
        let value = retry_read(3, || {
            calls += 1;
            if calls < 3 { Err("clipboard busy".into()) } else { Ok(Some("demo".into())) }
        }).unwrap();
        assert_eq!(value.as_deref(), Some("demo"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn retry_read_returns_the_last_error() {
        let mut calls = 0;
        let error = retry_read(3, || { calls += 1; Err("clipboard busy".into()) }).unwrap_err();
        assert_eq!(error, "clipboard busy");
        assert_eq!(calls, 3);
    }
}
```

`state.rs` 固定判重、原文、上限和关闭清理：

```rust
#[cfg(test)]
mod tests {
    use super::{CardRegistry, ResolveCard, MAX_CARDS, MAX_TEXT_BYTES};

    #[test]
    fn normalizes_only_for_source_deduplication() {
        let mut registry = CardRegistry::default();
        let ResolveCard::Create { label, .. } = registry.resolve(" demo\r\nline ").unwrap() else { panic!() };
        assert_eq!(registry.take_pending(&label).as_deref(), Some(" demo\r\nline "));
        assert!(matches!(registry.resolve("demo\nline"), Ok(ResolveCard::Focus { .. })));
    }

    #[test]
    fn reports_the_recent_card_at_the_limit() {
        let mut registry = CardRegistry::default();
        for index in 0..MAX_CARDS { registry.resolve(&format!("card-{index}")).unwrap(); }
        assert_eq!(registry.resolve("overflow").unwrap_err().recent_label(), Some("reference-card-6"));
    }

    #[test]
    fn rejects_empty_and_oversized_text() {
        let mut registry = CardRegistry::default();
        assert_eq!(registry.resolve(" \n ").unwrap_err().to_string(), "剪贴板中没有可用文本");
        assert_eq!(registry.resolve(&"a".repeat(MAX_TEXT_BYTES + 1)).unwrap_err().to_string(), "参考文本不能超过 8 MiB");
    }

    #[test]
    fn removing_a_window_allows_the_same_source_again() {
        let mut registry = CardRegistry::default();
        let ResolveCard::Create { label, .. } = registry.resolve("demo").unwrap() else { panic!() };
        registry.take_pending(&label);
        assert!(matches!(registry.resolve("demo"), Ok(ResolveCard::Focus { .. })));
        registry.remove_label(&label);
        assert!(matches!(registry.resolve("demo"), Ok(ResolveCard::Create { .. })));
    }
}
```

`position.rs` 固定负坐标显示器和 clamp：

```rust
#[cfg(test)]
mod tests {
    use super::{card_position, PhysicalRect, PhysicalSize};

    #[test]
    fn cascades_inside_a_negative_coordinate_work_area() {
        let work = PhysicalRect { x: -1920, y: 0, width: 1920, height: 1040 };
        let size = PhysicalSize { width: 560, height: 360 };
        let first = card_position(work, size, 0);
        let sixth = card_position(work, size, 5);
        assert!(first.0 >= -1920 && first.0 + 560 <= 0);
        assert!(sixth.0 >= -1920 && sixth.0 + 560 <= 0);
        assert!(sixth.1 >= 0 && sixth.1 + 360 <= 1040);
        assert_ne!(first, sixth);
    }
}
```

- [ ] **Step 2: 运行 Rust 测试并确认 RED**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml clipboard -- --nocapture`

Expected: FAIL，`clipboard` 模块不存在。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture`

Expected: FAIL，注册表和定位模块不存在。

- [ ] **Step 3: 实现共享 Windows Unicode 剪贴板读取**

`clipboard.rs` 使用 RAII 保证所有路径关闭剪贴板：

```rust
use std::time::Duration;

#[cfg(windows)]
pub(crate) struct ClipboardGuard;

#[cfg(windows)]
impl ClipboardGuard {
    pub(crate) fn open() -> Result<Self, String> {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::System::DataExchange::OpenClipboard;
        if unsafe { OpenClipboard(HWND::default()) } == 0 { return Err("打开剪贴板失败".into()); }
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ClipboardGuard {
    fn drop(&mut self) { unsafe { windows_sys::Win32::System::DataExchange::CloseClipboard(); } }
}

#[cfg(windows)]
pub(crate) fn read_unicode_text_from_open_clipboard() -> Result<Option<String>, String> {
    use windows_sys::Win32::System::DataExchange::{GetClipboardData, IsClipboardFormatAvailable};
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 { return Ok(None); }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() { return Err("读取剪贴板文本失败".into()); }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() { return Err("锁定剪贴板文本失败".into()); }
        let mut len = 0usize;
        while *ptr.add(len) != 0 { len += 1; }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(handle);
        Ok(Some(text))
    }
}

#[cfg(not(windows))]
pub(crate) fn read_unicode_text_from_open_clipboard() -> Result<Option<String>, String> { Ok(None) }

fn retry_read<F>(attempts: usize, mut read: F) -> Result<Option<String>, String>
where F: FnMut() -> Result<Option<String>, String>,
{
    let mut last_error = None;
    for attempt in 0..attempts {
        match read() { Ok(value) => return Ok(value), Err(error) => last_error = Some(error) }
        if attempt + 1 < attempts { std::thread::sleep(Duration::from_millis(20)); }
    }
    Err(last_error.unwrap_or_else(|| "读取剪贴板失败".into()))
}

pub(crate) fn read_unicode_text_with_retry() -> Result<Option<String>, String> {
    retry_read(3, || {
        #[cfg(windows)]
        let _guard = ClipboardGuard::open()?;
        read_unicode_text_from_open_clipboard()
    })
}
```

`inbox.rs` 在两个 `#[cfg(windows)]` 函数内部导入共享 guard，避免非 Windows 构建引用 Windows 专用类型。`copy_image_file_to_clipboard()` 删除本地 `ClipboardGuard`、`OpenClipboard` 和 `CloseClipboard` 导入，改为：

```rust
use crate::clipboard::ClipboardGuard;
use windows_sys::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
```

删除函数内的 `struct ClipboardGuard` 及其 `open`/`Drop` 实现；后面的 `let _guard = ClipboardGuard::open()?;`、`EmptyClipboard()` 和 `SetClipboardData()` 调用继续使用共享 guard。

`read_clipboard_candidate()` 同样删除本地 guard、`OpenClipboard`、`CloseClipboard` 和 `read_utf16_handle()`，在函数内部导入并改为：

```rust
use crate::clipboard::{ClipboardGuard, read_unicode_text_from_open_clipboard};

fn read_text() -> Result<Option<CaptureCandidate>, String> {
    let Some(text) = read_unicode_text_from_open_clipboard()? else { return Ok(None) };
    if text.trim().is_empty() { return Ok(None); }
    Ok(Some(build_text_candidate("text", text.clone(), text.trim().to_string())))
}
```

保留 Inbox 现有的一次打开、多格式读取和优先级，不改变图片、文件、HTML、RTF 或 unknown 行为。

- [ ] **Step 4: 实现注册表和定位纯函数**

`state.rs`：

```rust
use std::collections::HashMap;

pub(crate) const MAX_CARDS: usize = 6;
pub(crate) const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveCard { Focus { label: String }, Create { label: String, ordinal: usize } }
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ResolveError { message: String, recent_label: Option<String> }
impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.message) }
}
impl ResolveError { pub(crate) fn recent_label(&self) -> Option<&str> { self.recent_label.as_deref() } }

struct CardRecord { label: String, last_used: u64, pending_text: Option<String> }
#[derive(Default)]
pub(crate) struct CardRegistry { records: HashMap<String, CardRecord>, next_id: u64, usage: u64 }

fn source_hash(text: &str) -> String {
    blake3::hash(text.replace("\r\n", "\n").trim().as_bytes()).to_hex().to_string()
}

impl CardRegistry {
    pub(crate) fn resolve(&mut self, text: &str) -> Result<ResolveCard, ResolveError> {
        if text.trim().is_empty() {
            return Err(ResolveError { message: "剪贴板中没有可用文本".into(), recent_label: None });
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(ResolveError { message: "参考文本不能超过 8 MiB".into(), recent_label: None });
        }
        self.usage += 1;
        let hash = source_hash(text);
        if let Some(record) = self.records.get_mut(&hash) {
            record.last_used = self.usage;
            return Ok(ResolveCard::Focus { label: record.label.clone() });
        }
        if self.records.len() >= MAX_CARDS {
            let recent_label = self.records.values().max_by_key(|record| record.last_used).map(|record| record.label.clone());
            return Err(ResolveError { message: "最多同时打开 6 张参考卡，请先关闭一张".into(), recent_label });
        }
        self.next_id += 1;
        let label = format!("reference-card-{}", self.next_id);
        let ordinal = self.records.len();
        self.records.insert(hash, CardRecord { label: label.clone(), last_used: self.usage, pending_text: Some(text.to_string()) });
        Ok(ResolveCard::Create { label, ordinal })
    }
    pub(crate) fn take_pending(&mut self, label: &str) -> Option<String> {
        self.records.values_mut().find(|record| record.label == label)?.pending_text.take()
    }
    pub(crate) fn remove_label(&mut self, label: &str) { self.records.retain(|_, record| record.label != label); }
    pub(crate) fn retain_labels(&mut self, mut exists: impl FnMut(&str) -> bool) {
        self.records.retain(|_, record| exists(&record.label));
    }
}
```

`position.rs`：

```rust
#[derive(Clone, Copy)]
pub(crate) struct PhysicalRect { pub x: i32, pub y: i32, pub width: i32, pub height: i32 }
#[derive(Clone, Copy)]
pub(crate) struct PhysicalSize { pub width: i32, pub height: i32 }

pub(crate) fn card_position(work: PhysicalRect, size: PhysicalSize, ordinal: usize) -> (i32, i32) {
    let offset = (ordinal.min(5) as i32) * 28;
    let base_x = work.x + ((work.width - size.width).max(0) * 2 / 3);
    let base_y = work.y + ((work.height - size.height).max(0) / 3);
    let max_x = work.x + (work.width - size.width).max(0);
    let max_y = work.y + (work.height - size.height).max(0);
    ((base_x + offset).clamp(work.x, max_x), (base_y + offset).clamp(work.y, max_y))
}
```

`reference_card/mod.rs` 先声明 `mod position; mod state;`。`main.rs` 加 `mod clipboard; mod reference_card;`。

- [ ] **Step 5: 运行 Rust 定向测试确认 GREEN**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml clipboard -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml inbox -- --nocapture`

Expected: PASS，共享读取没有改变 Inbox candidate、hash 和抑制测试。

- [ ] **Step 6: 提交 Rust 纯核心**

```powershell
git add apps/desktop/src-tauri/src/clipboard.rs apps/desktop/src-tauri/src/reference_card apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/tools/inbox.rs
git commit -m "feat(reference-card): 建立剪贴板与会话核心"
```

---

### Task 4: 实现 Tauri 动态窗口管理和双入口后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs:12-26,380-590,790-810,1200-1460`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src/bridge/events.ts`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`

- [ ] **Step 1: 写窗口接线失败测试**

在 `reference_card/mod.rs` 增加：

```rust
#[cfg(test)]
mod wiring_tests {
    #[test]
    fn main_registers_reference_card_commands_and_hotkey() {
        let source = include_str!("../main.rs");
        assert!(source.contains("reference_card::reference_card_show"));
        assert!(source.contains("reference_card::reference_card_ready"));
        assert!(source.contains("name_owned == \"reference-card\""));
        assert!(source.contains("reference_card::on_window_closed(window.label())"));
    }

    #[test]
    fn capability_allows_dynamic_reference_card_labels() {
        let source = include_str!("../../capabilities/default.json");
        assert!(source.contains("\"reference-card-*\""));
    }

    #[tokio::test]
    async fn ready_wait_reports_success_and_timeout() {
        use std::time::Duration;
        use tokio::sync::oneshot;
        use super::wait_for_ready;

        let (sender, receiver) = oneshot::channel();
        sender.send(Ok(())).unwrap();
        assert_eq!(wait_for_ready(receiver, Duration::from_millis(20)).await, Ok(()));

        let (sender, receiver) = oneshot::channel();
        sender.send(Err("初始化事件发送失败".into())).unwrap();
        assert_eq!(
            wait_for_ready(receiver, Duration::from_millis(20)).await.unwrap_err(),
            "初始化事件发送失败",
        );

        let (_sender, receiver) = oneshot::channel();
        assert_eq!(
            wait_for_ready(receiver, Duration::from_millis(1)).await.unwrap_err(),
            "参考卡初始化超时",
        );
    }
}
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture`

Expected: FAIL，command、快捷键分支和 capability 尚未接入。

- [ ] **Step 3: 实现统一创建、聚焦和 ready 握手**

`reference_card/mod.rs` 的稳定公共边界：

```rust
mod position;
mod state;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::oneshot;
use position::{card_position, PhysicalRect, PhysicalSize};
use state::{CardRegistry, ResolveCard};

pub(crate) const REFERENCE_CARD_PREFIX: &str = "reference-card-";
pub(crate) const REFERENCE_CARD_TITLE: &str = "置顶参考";
const WIDTH: i32 = 560;
const HEIGHT: i32 = 360;
const READY_TIMEOUT: Duration = Duration::from_secs(5);
static REGISTRY: LazyLock<Mutex<CardRegistry>> = LazyLock::new(|| Mutex::new(CardRegistry::default()));
type ReadySender = oneshot::Sender<Result<(), String>>;
static READY_WAITERS: LazyLock<Mutex<HashMap<String, ReadySender>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferenceCardShowResult { outcome: &'static str, window_label: String }

#[derive(Serialize)]
struct ReferenceCardInitPayload { content: String }

fn reference_card_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External("http://localhost:5173/?view=reference-card".parse().expect("valid reference card dev url"))
    } else {
        WebviewUrl::App("index.html?view=reference-card".into())
    }
}

fn notify_error(app: &AppHandle, message: &str) {
    if let Err(error) = app.notification().builder().title(REFERENCE_CARD_TITLE).body(message).show() {
        eprintln!("reference-card notification failed: {error}; original error: {message}");
    }
}

async fn wait_for_ready(
    receiver: oneshot::Receiver<Result<(), String>>,
    timeout: Duration,
) -> Result<(), String> {
    match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("参考卡初始化通道已关闭".into()),
        Err(_) => Err("参考卡初始化超时".into()),
    }
}

fn signal_ready(label: &str, result: Result<(), String>) -> Result<(), String> {
    let sender = READY_WAITERS
        .lock()
        .map_err(|_| "参考卡 ready 状态锁定失败".to_string())?
        .remove(label)
        .ok_or_else(|| "参考卡 ready 请求不存在或已超时".to_string())?;
    sender.send(result).map_err(|_| "参考卡 ready 接收端已关闭".to_string())
}

fn cleanup_failed_card(app: &AppHandle, label: &str) {
    on_window_closed(label);
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }
}

async fn show_text(app: &AppHandle, text: String) -> Result<ReferenceCardShowResult, String> {
    let mut registry = REGISTRY.lock().map_err(|_| "参考卡状态锁定失败".to_string())?;
    registry.retain_labels(|label| app.get_webview_window(label).is_some());
    match registry.resolve(&text).map_err(|error| {
        if let Some(label) = error.recent_label() {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        error.to_string()
    })? {
        ResolveCard::Focus { label } => {
            let window = app.get_webview_window(&label).ok_or("参考卡窗口已失效")?;
            window.show().map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())?;
            Ok(ReferenceCardShowResult { outcome: "focused", window_label: label })
        }
        ResolveCard::Create { label, ordinal } => {
            drop(registry);
            let (sender, receiver) = oneshot::channel();
            READY_WAITERS
                .lock()
                .map_err(|_| "参考卡 ready 状态锁定失败".to_string())?
                .insert(label.clone(), sender);
            let window = WebviewWindowBuilder::new(app, &label, reference_card_url())
                .title(REFERENCE_CARD_TITLE)
                .inner_size(WIDTH as f64, HEIGHT as f64)
                .min_inner_size(360.0, 220.0)
                .decorations(false).resizable(true).always_on_top(true)
                .skip_taskbar(true).focused(false).visible(false)
                .build()
                .map_err(|error| {
                    cleanup_failed_card(app, &label);
                    format!("创建参考卡失败: {error}")
                })?;
            position_window(&window, ordinal);
            if let Err(error) = wait_for_ready(receiver, READY_TIMEOUT).await {
                cleanup_failed_card(app, &label);
                return Err(error);
            }
            Ok(ReferenceCardShowResult { outcome: "created", window_label: label })
        }
    }
}

fn position_window(window: &WebviewWindow, ordinal: usize) {
    let monitor = window.cursor_position().ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let size = PhysicalSize {
        width: (WIDTH as f64 * scale).round() as i32,
        height: (HEIGHT as f64 * scale).round() as i32,
    };
    let rect = PhysicalRect {
        x: work.position.x, y: work.position.y,
        width: work.size.width as i32, height: work.size.height as i32,
    };
    let (x, y) = card_position(rect, size, ordinal);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

```

两个 command 和全局入口都调用 `show_text`：

```rust
#[tauri::command]
pub(crate) async fn reference_card_show(
    app: AppHandle,
    text: String,
) -> Result<ReferenceCardShowResult, String> {
    show_text(&app, text).await
}

#[tauri::command]
pub(crate) fn reference_card_ready(window: WebviewWindow) -> Result<(), String> {
    let label = window.label().to_string();
    if !label.starts_with(REFERENCE_CARD_PREFIX) { return Err("无效的参考卡窗口".into()); }
    let content = REGISTRY.lock().map_err(|_| "参考卡状态锁定失败".to_string())?
        .take_pending(&label);
    let Some(content) = content else {
        let error = "参考卡初始化数据不存在".to_string();
        let _ = signal_ready(&label, Err(error.clone()));
        cleanup_failed_card(window.app_handle(), &label);
        return Err(error);
    };
    let result = (|| -> Result<(), String> {
        window
            .emit(crate::events::EVENT_REFERENCE_CARD_INIT, ReferenceCardInitPayload { content })
            .map_err(|error| format!("发送参考卡初始化内容失败: {error}"))?;
        window.show().map_err(|error| format!("显示参考卡失败: {error}"))?;
        window.set_focus().map_err(|error| format!("聚焦参考卡失败: {error}"))?;
        Ok(())
    })();
    let signal_result = signal_ready(&label, result.clone());
    if let Err(error) = result {
        cleanup_failed_card(window.app_handle(), &label);
        return Err(error);
    }
    if let Err(error) = signal_result {
        cleanup_failed_card(window.app_handle(), &label);
        return Err(error);
    }
    Ok(())
}

pub(crate) fn show_from_clipboard(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match crate::clipboard::read_unicode_text_with_retry() {
            Ok(Some(text)) => show_text(&app, text).await.map(|_| ()),
            Ok(None) => Err("剪贴板中没有可用文本".into()),
            Err(error) => Err(error),
        };
        if let Err(error) = result { notify_error(&app, &error); }
    });
}

pub(crate) fn on_window_closed(label: &str) {
    if !label.starts_with(REFERENCE_CARD_PREFIX) { return; }
    if let Ok(mut registry) = REGISTRY.lock() { registry.remove_label(label); }
    if let Ok(mut waiters) = READY_WAITERS.lock() { waiters.remove(label); }
}
```

- [ ] **Step 4: 接入 main、事件和 capability**

`events.rs`、`events::ALL` 与 `bridge/events.ts` 同步增加：

```rust
pub const EVENT_REFERENCE_CARD_INIT: &str = "reference-card://init";
```

```ts
REFERENCE_CARD_INIT: "reference-card://init",
```

`default.json` 的 windows 数组加入：

```json
"reference-card-*"
```

`main.rs::expected_window_title` 对动态标签返回标题：

```rust
_ if window_label.starts_with(reference_card::REFERENCE_CARD_PREFIX) => {
    Some(reference_card::REFERENCE_CARD_TITLE)
}
```

`sync_all_shortcuts` 加入：

```rust
if name_owned == "reference-card" {
    reference_card::show_from_clipboard(app_handle);
    return;
}
```

`CloseRequested` 的非主窗口分支先清理参考卡：

```rust
if window.label() != MAIN_WINDOW_LABEL {
    reference_card::on_window_closed(window.label());
    tools::access_path_diagnostics::runtime::on_window_closed(window.label());
    return;
}
```

同时在 match 中处理最终销毁，覆盖构建失败、系统关闭和异常退出路径：

```rust
WindowEvent::Destroyed => reference_card::on_window_closed(window.label()),
```

`invoke_handler` 注册：

```rust
reference_card::reference_card_show,
reference_card::reference_card_ready,
```

- [ ] **Step 5: 运行 Rust 测试、契约测试和 check**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture`

Expected: PASS，动态窗口接线、capability 和纯状态测试通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS，Rust/TypeScript 事件常量同步。

Run: `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

Expected: PASS，Tauri builder、notification、window 注入和 Win32 读取均编译。

- [ ] **Step 6: 提交后端窗口能力**

```powershell
git add apps/desktop/src-tauri/src/reference_card apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src/bridge/events.ts apps/desktop/src-tauri/capabilities/default.json
git commit -m "feat(reference-card): 添加置顶多窗口运行时"
```

---

### Task 5: 实现可编辑参考卡窗口和 Monaco 交互

**Files:**

- Create: `apps/desktop/src/types/reference-card.ts`
- Create: `apps/desktop/src/ReferenceCardApp.ts`
- Create: `apps/desktop/src/components/ReferenceCard.vue`
- Create: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
- Modify: `apps/desktop/src/components/MonacoPane.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/main.ts`

- [ ] **Step 1: 写前端窗口契约失败测试**

创建 `ReferenceCard.contract.test.ts`：

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const root = new URL("../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf-8");

describe("ReferenceCard window wiring", () => {
  const component = read("components/ReferenceCard.vue");
  const main = read("main.ts");
  const bridge = read("bridge/tauri.ts");
  const monaco = read("components/MonacoPane.vue");

  it("mounts a dedicated card view", () => {
    expect(main).toContain('currentView === "reference-card"');
    expect(main).toContain('import("./ReferenceCardApp")');
  });

  it("subscribes before announcing ready", () => {
    expect(component.indexOf("listen<ReferenceCardInitPayload>")).toBeGreaterThan(-1);
    expect(component.indexOf("listen<ReferenceCardInitPayload>")).toBeLessThan(
      component.indexOf("referenceCardReady()"),
    );
    expect(bridge).toContain('invoke("reference_card_ready")');
  });

  it("uses Monaco and keeps transient content out of persistence", () => {
    expect(component).toContain("<MonacoPane");
    expect(component).toContain("data-tauri-drag-region");
    expect(component).toContain("suppressClipboardCapture(content.value)");
    expect(component).not.toContain("localStorage");
    expect(component).not.toContain("setSetting(");
  });

  it("adds explicit word-wrap and focus APIs without changing defaults", () => {
    expect(monaco).toContain("wordWrap?: boolean");
    expect(monaco).toContain("wordWrap: false");
    expect(monaco).toContain("function focusEditor()");
    expect(monaco).toContain("defineExpose({ formatDocument, focusLine, focusText, focusEditor })");
  });

  it("reports Monaco initialization and language failures with context", () => {
    expect(monaco).toContain('(event: "error", message: string): void');
    expect(monaco).toContain("Monaco 初始化失败");
    expect(monaco).toContain("切换 Monaco 语言失败");
    expect(component).toContain('@error="handleEditorError"');
  });
});
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts`

Expected: FAIL，参考卡组件、挂载入口和 Monaco 新 API 尚不存在。

- [ ] **Step 3: 扩展 MonacoPane 的显式小窗口 API**

`MonacoPane.vue` props 保持现有默认行为：

```ts
const props = withDefaults(
  defineProps<{
    modelValue: string;
    language?: string;
    readOnly?: boolean;
    ariaLabel?: string;
    wordWrap?: boolean;
  }>(),
  {
    language: "plaintext",
    readOnly: false,
    ariaLabel: "代码编辑器",
    wordWrap: false,
  },
);

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
  (event: "error", message: string): void;
}>();
```

把现有 `onMounted` 替换为带明确上下文的初始化，并加入 `wordWrap`：

```ts
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

onMounted(() => {
  try {
    editor = monaco.editor.create(container.value as HTMLElement, {
      value: props.modelValue,
      language: props.language,
      theme: "vs",
      readOnly: props.readOnly,
      ariaLabel: props.ariaLabel,
      automaticLayout: true,
      minimap: { enabled: false },
      wordWrap: props.wordWrap ? "on" : "off",
      scrollbar: { alwaysConsumeMouseWheel: false },
      guides: { indentation: true, bracketPairs: true },
    });
    editor.onDidChangeModelContent(() => {
      if (suppressEmit || !editor) return;
      emit("update:modelValue", editor.getValue());
    });
  } catch (error) {
    emit("error", `Monaco 初始化失败：${errorMessage(error)}`);
  }
});
```

把语言 watch 替换为显式失败事件，再新增 word-wrap watch 和 focus API：

```ts
watch(
  () => props.language,
  (language) => {
    if (!editor) return;
    const model = editor.getModel();
    if (!model) return;
    try {
      monaco.editor.setModelLanguage(model, language ?? "plaintext");
    } catch (error) {
      emit("error", `切换 Monaco 语言失败：${errorMessage(error)}`);
    }
  },
);

watch(
  () => props.wordWrap,
  (enabled) => editor?.updateOptions({ wordWrap: enabled ? "on" : "off" }),
);

function focusEditor() {
  editor?.focus();
}

defineExpose({ formatDocument, focusLine, focusText, focusEditor });
```

- [ ] **Step 4: 创建 payload、bridge 和挂载入口**

`types/reference-card.ts`：

```ts
export interface ReferenceCardInitPayload {
  content: string;
}
```

`bridge/tauri.ts` 增加：

```ts
export async function referenceCardReady(): Promise<void> {
  await invoke("reference_card_ready");
}
```

`ReferenceCardApp.ts`：

```ts
import { createApp } from "vue";
import ReferenceCard from "./components/ReferenceCard.vue";

export default function mountReferenceCardApp() {
  createApp(ReferenceCard).mount("#app");
}
```

`main.ts` 在 quick-capture 分支后增加：

```ts
} else if (currentView === "reference-card") {
  import("./ReferenceCardApp").then(({ default: mount }) => mount());
```

- [ ] **Step 5: 实现 ReferenceCard 组件**

使用原生轻量工具栏，避免为独立小窗口增加额外 Element Plus 组件状态：

```vue
<template>
  <div class="reference-card">
    <header class="card-toolbar">
      <span class="drag-grip" data-tauri-drag-region>•••</span>
      <span class="card-label" data-tauri-drag-region>置顶参考</span>
      <span class="toolbar-spacer" data-tauri-drag-region />
      <select v-model="language" class="language-select" aria-label="代码语言">
        <option v-for="option in MONACO_LANGUAGE_OPTIONS" :key="option" :value="option">
          {{ option }}
        </option>
      </select>
      <button
        type="button"
        class="toolbar-button"
        :class="{ active: wordWrap }"
        @click="wordWrap = !wordWrap"
      >
        自动换行
      </button>
      <button type="button" class="toolbar-button" @click="copyAll">复制全部</button>
      <button
        type="button"
        class="toolbar-button close-button"
        aria-label="关闭"
        @click="closeCard"
      >
        ×
      </button>
    </header>
    <div v-if="errorMessage" class="card-error" role="alert">{{ errorMessage }}</div>
    <MonacoPane
      ref="editorRef"
      v-model="content"
      class="card-editor"
      :language="language"
      :word-wrap="wordWrap"
      aria-label="置顶参考卡编辑器"
      @error="handleEditorError"
    />
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { APP_EVENTS } from "../bridge/events";
import { referenceCardReady, suppressClipboardCapture } from "../bridge/tauri";
import type { ReferenceCardInitPayload } from "../types/reference-card";
import { detectClipboardMonacoLanguage, MONACO_LANGUAGE_OPTIONS } from "../utils/monacoLanguages";
import MonacoPane from "./MonacoPane.vue";

interface MonacoPaneApi {
  focusEditor(): void;
}

const content = ref("");
const language = ref("plaintext");
const wordWrap = ref(true);
const errorMessage = ref("");
const editorRef = ref<MonacoPaneApi | null>(null);
let unlistenInit: UnlistenFn | null = null;

onMounted(async () => {
  try {
    unlistenInit = await listen<ReferenceCardInitPayload>(
      APP_EVENTS.REFERENCE_CARD_INIT,
      async ({ payload }) => {
        content.value = payload.content;
        language.value = detectClipboardMonacoLanguage(payload.content);
        await nextTick();
        editorRef.value?.focusEditor();
      },
    );
    await referenceCardReady();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
});

onBeforeUnmount(() => unlistenInit?.());

async function copyAll() {
  try {
    await suppressClipboardCapture(content.value);
    await navigator.clipboard.writeText(content.value);
    errorMessage.value = "";
  } catch (error) {
    errorMessage.value = `复制失败：${error instanceof Error ? error.message : String(error)}`;
  }
}

async function closeCard() {
  try {
    await getCurrentWindow().close();
  } catch (error) {
    errorMessage.value = `关闭失败：${error instanceof Error ? error.message : String(error)}`;
  }
}

function handleEditorError(message: string) {
  errorMessage.value = message;
  console.error(`[reference-card] ${message}`);
}
</script>
```

组件样式完整写为：

```css
<style scoped>
.reference-card {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
  color: #1f2937;
  background: #fff;
  border: 1px solid #d8dee8;
}

.card-toolbar {
  height: 38px;
  flex: 0 0 38px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  box-sizing: border-box;
  background: #f7f8fa;
  border-bottom: 1px solid #e4e7ed;
  user-select: none;
}

.drag-grip,
.card-label,
.toolbar-spacer {
  align-self: stretch;
  display: flex;
  align-items: center;
  cursor: move;
}

.drag-grip { color: #909399; letter-spacing: 1px; }
.card-label { font-size: 13px; font-weight: 600; white-space: nowrap; }
.toolbar-spacer { flex: 1 1 auto; min-width: 8px; }

.language-select,
.toolbar-button {
  height: 26px;
  box-sizing: border-box;
  border: 1px solid #dcdfe6;
  border-radius: 5px;
  color: #303133;
  background: #fff;
  font: inherit;
}

.language-select { max-width: 118px; padding: 0 6px; }
.toolbar-button { padding: 0 8px; cursor: pointer; }
.toolbar-button:hover,
.toolbar-button.active { color: #2563eb; border-color: #93b4f5; background: #eff6ff; }
.close-button { width: 28px; padding: 0; font-size: 18px; }
.close-button:hover { color: #dc2626; border-color: #f3a6a6; background: #fff1f1; }

.card-error {
  flex: 0 0 auto;
  padding: 5px 10px;
  color: #b42318;
  background: #fff1f0;
  border-bottom: 1px solid #ffccc7;
  font-size: 12px;
}

.card-editor {
  flex: 1 1 auto;
  min-height: 0;
  border: 0;
  border-radius: 0;
}
</style>
```

- [ ] **Step 6: 运行测试、类型检查和渲染层构建**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts src/utils/monacoLanguages.test.ts`

Expected: PASS，ready 顺序、瞬态边界、Monaco API 和语言识别通过。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS，Monaco Worker 和 reference-card 动态入口进入本地构建，无公网资源。

- [ ] **Step 7: 提交参考卡 UI**

```powershell
git add apps/desktop/src/types/reference-card.ts apps/desktop/src/ReferenceCardApp.ts apps/desktop/src/components/ReferenceCard.vue apps/desktop/src/components/ReferenceCard.contract.test.ts apps/desktop/src/components/MonacoPane.vue apps/desktop/src/bridge/tauri.ts apps/desktop/src/main.ts
git commit -m "feat(reference-card): 添加 Monaco 置顶卡片界面"
```

---

### Task 6: 接入默认全局快捷键和设置页

**Files:**

- Modify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
- Modify: `apps/desktop/src/components/SettingsPanel.vue:95-142,500-675`
- Modify: `apps/desktop/src/App.vue:105-116,205-230,318-355`

- [ ] **Step 1: 扩展快捷键接线失败测试**

在 `ReferenceCard.contract.test.ts` 增加：

```ts
describe("ReferenceCard shortcut settings", () => {
  const app = read("App.vue");
  const settings = read("components/SettingsPanel.vue");

  it("loads and registers the default shortcut", () => {
    expect(app).toContain('getSetting("hotkey_reference_card") ?? "Ctrl+Alt+Space"');
    expect(app).toContain('registerNamedHotkey("reference-card", savedReferenceCardHotkey)');
  });

  it("includes the shortcut in conflict, save and clear flows", () => {
    expect(settings).toContain('{ key: "referenceCardHotkeyInput" as const, label: "置顶参考卡" }');
    expect(settings).toContain('registerNamedHotkey("reference-card", referenceCard)');
    expect(settings).toContain('setSetting("hotkey_reference_card", referenceCard)');
    expect(settings).toContain('unregisterNamedHotkey("reference-card")');
    expect(settings).toContain('emit("update:referenceCardHotkeyInput", "")');
  });
});
```

- [ ] **Step 2: 运行契约测试并确认 RED**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts`

Expected: FAIL，App 和 SettingsPanel 尚未声明 `referenceCardHotkeyInput`。

- [ ] **Step 3: 在 App 中加载、注册和传递快捷键**

与 quick-capture/spotlight 状态相邻增加：

```ts
const referenceCardHotkeyInput = ref("");
```

Settings props 和 update handler 增加：

```ts
referenceCardHotkeyInput: referenceCardHotkeyInput.value,
"onUpdate:referenceCardHotkeyInput": (value: string) => {
  referenceCardHotkeyInput.value = value;
},
```

`onMounted` 在 quick-capture 后、Spotlight 前加载默认值：

```ts
const savedReferenceCardHotkey = getSetting("hotkey_reference_card") ?? "Ctrl+Alt+Space";
referenceCardHotkeyInput.value = savedReferenceCardHotkey;
if (savedReferenceCardHotkey) {
  try {
    await registerNamedHotkey("reference-card", savedReferenceCardHotkey);
  } catch {
    /* ignore in non-Tauri env */
  }
}
```

- [ ] **Step 4: 在 SettingsPanel 接入录制、冲突、保存和清空**

在 Spotlight 设置项前加入：

```vue
<div class="setting-item">
  <div class="setting-label"><span class="label-text">置顶参考卡</span></div>
  <div class="setting-control">
    <ShortcutRecorder
      :model-value="referenceCardHotkeyInput"
      :check-conflict="makeConflictChecker('referenceCardHotkeyInput')"
      @update:model-value="emit('update:referenceCardHotkeyInput', $event)"
    />
  </div>
</div>
```

props、emits 和 HOTKEY_FIELDS 同步加入：

```ts
referenceCardHotkeyInput: string;
(event: "update:referenceCardHotkeyInput", value: string): void;
{ key: "referenceCardHotkeyInput" as const, label: "置顶参考卡" },
```

`saveHotkeySettings` 加入：

```ts
const referenceCard = props.referenceCardHotkeyInput.trim();
await registerNamedHotkey("reference-card", referenceCard);
setSetting("hotkey_reference_card", referenceCard);
```

`clearHotkeySettings` 同步加入：

```ts
emit("update:referenceCardHotkeyInput", "");
await unregisterNamedHotkey("reference-card");
setSetting("hotkey_reference_card", "");
```

- [ ] **Step 5: 运行契约测试和类型检查确认 GREEN**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts`

Expected: PASS，默认值、命名快捷键、冲突、保存和清空全部接线。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS，SettingsPanel props/emits 与 App 动态 component props 一致。

- [ ] **Step 6: 提交快捷键设置**

```powershell
git add apps/desktop/src/App.vue apps/desktop/src/components/SettingsPanel.vue apps/desktop/src/components/ReferenceCard.contract.test.ts
git commit -m "feat(reference-card): 接入全局快捷键设置"
```

---

### Task 7: 完成跨层回归、经验沉淀和真实窗口验收

**Files:**

- Modify: `docs/experience/architecture.md`
- Modify: `docs/experience/spotlight-and-launcher.md`
- Modify: `docs/experience/vault-and-inbox.md`
- Verify: `docs/superpowers/specs/2026-07-25-reference-card-design.md`

- [ ] **Step 1: 运行完整定向回归**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/monacoLanguages.test.ts src/spotlight/clipboard-suggestions.test.ts src/spotlight/providers/suggestion.test.ts src/components/ReferenceCard.contract.test.ts src/spotlight/search.test.ts src/utils/clipboard-detect.test.ts`

Expected: PASS，语言、建议顺序、执行路由、窗口接线和原剪贴板检测全部通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml clipboard -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml inbox -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS。

- [ ] **Step 2: 若回归失败，先补最小测试再修根因**

只修复本功能引入的契约、判重、窗口生命周期、Spotlight 排序或共享剪贴板读取问题。回归断言必须落到对应纯模块，例如：

```rust
assert!(matches!(registry.resolve("same"), Ok(ResolveCard::Focus { .. })));
assert_eq!(registry.resolve(" \n ").unwrap_err().to_string(), "剪贴板中没有可用文本");
```

```ts
expect(buildClipboardSuggestionItems("unknown")[0].payload?.suggestionAction).toEqual({
  kind: "open-reference-card",
  text: "unknown",
});
```

Run: 重跑最先失败的精确测试命令。

Expected: 新回归测试先 FAIL，根因修复后 PASS；不得通过调整断言、静默 fallback 或扩大文本持久化范围让测试假绿。

- [ ] **Step 3: 沉淀三条直接相关经验**

在 `architecture.md` 目录和正文增加：

```md
## 动态 Tauri 窗口使用前端 ready 握手

动态窗口使用稳定 label 前缀和 capability 通配模式。窗口先以 `visible = false` 创建，前端完成事件订阅后再调用 ready command；后端随后发送初始化 payload 并显示窗口，避免 page-load 与 Vue listener 之间的竞态。ready 超时、窗口构建失败和关闭事件都必须清理内存注册表。
```

在 `spotlight-and-launcher.md` 增加：

```md
## 剪贴板建议使用判别式动作

同一剪贴板内容可以同时产生领域工具建议和通用参考动作。建议 payload 使用 `open-tool` / `open-reference-card` 判别字段，不能用虚构工具 ID 复用主窗口导航。现有高置信度工具建议保持首位，通用参考结果通过独立 searchFields 支持“参考、置顶、卡片”等查询。
```

在 `vault-and-inbox.md` 增加：

```md
## 临时参考内容保持会话级

置顶参考卡正文只由卡片渲染进程持有，不写数据库、设置或 localStorage。卡片“复制全部”仍需先设置一次性 Inbox 回采抑制，再写系统剪贴板；关闭卡片或退出进程即丢弃内容。
```

同步三个经验文件的目录锚点和使用次数。

- [ ] **Step 4: 运行最终自动化验证**

Run: `pnpm test`

Expected: PASS，全部前端单元和源契约测试通过。

Run: `pnpm typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS，所有 Monaco Worker 和动态入口本地打包。

Run: `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

Expected: PASS，无新增编译错误或生产构建 dead-code 警告。

Run: `git diff --check`

Expected: 无输出。

Run: `rg -n "T[B]D|T[O]DO|implement[ -]later|localStorage.*reference|setSetting.*reference.*content" apps/desktop/src apps/desktop/src-tauri/src docs/experience`

Expected: 不出现实施占位，也不出现参考卡正文持久化。

- [ ] **Step 5: 在获得用户明确许可后做真实 Tauri 冒烟**

不要自动运行 `pnpm dev`。获得许可后运行 `pnpm dev`，按以下矩阵验证；未获许可时在最终交付中明确标注“未运行 UI 冒烟”，不能把构建通过表述成运行时通过：

```text
快捷键 + 普通文本             -> 鼠标所在显示器创建 560×360 置顶卡
快捷键 + 相同原文             -> 聚焦已有卡，不新增
编辑已有卡 + 再触发原始文本    -> 仍聚焦该卡，编辑内容保留
快捷键 + 不同文本 × 3          -> 三张卡错位出现，可独立拖动缩放
第 7 张卡                      -> 聚焦最近卡并明确提示，不覆盖
空/非文本/超过 8 MiB           -> 不建卡并显示原因
Spotlight + JSON               -> 原格式化建议第一，参考卡建议第二
Spotlight + 未识别文本          -> 参考卡建议第一
搜索“参考/置顶/卡片”           -> 可筛出参考卡结果
从 Spotlight 创建             -> 关闭 Spotlight，不唤起主窗口
语言切换/查找/自动换行         -> Monaco 行为正常
复制全部                       -> 剪贴板写入成功，Inbox 不回采
关闭卡片/退出 LazyCat          -> 内容不恢复
副屏负坐标/不同 DPI            -> 窗口完整位于工作区内
```

- [ ] **Step 6: 提交经验文档**

```powershell
git add docs/experience/architecture.md docs/experience/spotlight-and-launcher.md docs/experience/vault-and-inbox.md
git commit -m "docs: 沉淀置顶参考卡窗口经验"
```

---

## 完成定义

- 全局 `reference-card` 快捷键和 Spotlight 使用同一个 Rust 创建函数。
- Spotlight 保留原智能工具建议顺序，未知文本仍能创建参考卡。
- 相同来源只聚焦已有卡；不同来源最多创建 6 张独立窗口。
- 窗口是 always-on-top、可拖动、可缩放、skip-taskbar，并在鼠标显示器内错位摆放。
- Monaco 支持直接编辑、语言选择、行号、高亮、查找和自动换行。
- 空、非文本、超限、剪贴板占用、窗口构建和 ready 超时都显式失败。
- 参考卡正文不进入 SQLite、设置、localStorage、文件或 Inbox。
- 关闭卡片和退出应用会清理运行时内容；复制全部使用 Inbox 回采抑制。
- 定向测试、`pnpm test`、`pnpm typecheck`、`build:web`、`cargo check` 和 `git diff --check` 全部通过。
- 真实 always-on-top、聚焦、多显示器和 DPI 行为只有在实际 UI 冒烟后才能声明通过。

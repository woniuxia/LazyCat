<template>
  <div ref="container" class="db-sql-editor"></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import monaco from "../../utils/monaco-setup";
import { statementAtCursor, type SqlDialect } from "../../utils/dbSqlClassify";

/**
 * SQL 编辑器：monaco + 基于结构树的静态补全 + Ctrl/Cmd+Enter 执行。
 * 补全 provider 全局注册一次，按 model URI 查各编辑器实例的补全词表。
 */

const props = withDefaults(
  defineProps<{
    modelValue: string;
    dialect: SqlDialect;
    /** 表名 / 列名补全词表（结构树加载后更新） */
    completions?: string[];
  }>(),
  { completions: () => [] }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
  (e: "execute"): void;
}>();

const container = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let suppressEmit = false;

/** modelUri -> 补全词表（模块级，供全局 provider 查询） */
const completionRegistry: Map<string, string[]> = ((): Map<string, string[]> => {
  const key = "__lazycatDbSqlCompletions";
  const holder = globalThis as Record<string, unknown>;
  if (!holder[key]) holder[key] = new Map<string, string[]>();
  return holder[key] as Map<string, string[]>;
})();

let providerRegistered = false;
function ensureCompletionProvider(): void {
  const key = "__lazycatDbSqlProvider";
  const holder = globalThis as Record<string, unknown>;
  if (holder[key] || providerRegistered) return;
  holder[key] = true;
  providerRegistered = true;
  monaco.languages.registerCompletionItemProvider("sql", {
    provideCompletionItems(model, position) {
      const words = completionRegistry.get(model.uri.toString()) ?? [];
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(
        position.lineNumber,
        word.startColumn,
        position.lineNumber,
        word.endColumn
      );
      return {
        suggestions: words.map((w) => ({
          label: w,
          kind: monaco.languages.CompletionItemKind.Field,
          insertText: w,
          range,
        })),
      };
    },
  });
}

onMounted(() => {
  ensureCompletionProvider();
  editor = monaco.editor.create(container.value as HTMLElement, {
    value: props.modelValue,
    language: "sql",
    theme: "vs",
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 13,
    scrollBeyondLastLine: false,
    wordWrap: "on",
  });
  syncCompletions();

  editor.onDidChangeModelContent(() => {
    if (suppressEmit || !editor) return;
    emit("update:modelValue", editor.getValue());
  });

  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
    emit("execute");
  });
});

watch(
  () => props.modelValue,
  (value) => {
    if (!editor || value === editor.getValue()) return;
    suppressEmit = true;
    editor.setValue(value);
    suppressEmit = false;
  }
);

watch(() => props.completions, syncCompletions, { deep: true });

function syncCompletions(): void {
  const model = editor?.getModel();
  if (model) {
    completionRegistry.set(model.uri.toString(), props.completions);
  }
}

/** 取"选中文本，否则光标所在语句"；供执行入口使用。 */
function getExecutableSql(): string | null {
  if (!editor) return null;
  const selection = editor.getSelection();
  const model = editor.getModel();
  if (!model) return null;
  if (selection && !selection.isEmpty()) {
    const text = model.getValueInRange(selection).trim();
    return text || null;
  }
  const position = editor.getPosition();
  if (!position) return null;
  const offset = model.getOffsetAt(position);
  return statementAtCursor(model.getValue(), offset, props.dialect);
}

function insertText(text: string): void {
  if (!editor) return;
  const position = editor.getPosition();
  editor.executeEdits("insert", [
    {
      range: position
        ? new monaco.Range(position.lineNumber, position.column, position.lineNumber, position.column)
        : new monaco.Range(1, 1, 1, 1),
      text,
    },
  ]);
  editor.focus();
}

onBeforeUnmount(() => {
  const model = editor?.getModel();
  if (model) {
    completionRegistry.delete(model.uri.toString());
  }
  editor?.dispose();
  editor = null;
});

defineExpose({ getExecutableSql, insertText });
</script>

<style scoped>
.db-sql-editor {
  width: 100%;
  height: 100%;
  min-height: 120px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 10px);
  overflow: hidden;
}
</style>

import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import eslintConfigPrettier from "eslint-config-prettier";

export default [
  // 全局忽略
  {
    ignores: [
      "**/dist/**",
      "**/dist-renderer/**",
      "**/dist-ts/**",
      "**/node_modules/**",
      "**/.tauri/**",
      "**/src-tauri/**",
      "resources/**",
      "auto-imports.d.ts",
      "components.d.ts",
    ],
  },

  // JS 基础规则
  js.configs.recommended,

  // TypeScript 规则
  ...tseslint.configs.recommended,

  // Vue 规则
  ...pluginVue.configs["flat/recommended"],

  // 浏览器 + Node 全局变量（desktop app 两端都用）
  {
    languageOptions: {
      globals: {
        // Browser
        document: "readonly",
        window: "readonly",
        navigator: "readonly",
        HTMLElement: "readonly",
        MutationObserver: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
        requestAnimationFrame: "readonly",
        cancelAnimationFrame: "readonly",
        fetch: "readonly",
        URL: "readonly",
        Blob: "readonly",
        File: "readonly",
        FileReader: "readonly",
        FormData: "readonly",
        Event: "readonly",
        CustomEvent: "readonly",
        AbortController: "readonly",
        console: "readonly",
        localStorage: "readonly",
        matchMedia: "readonly",
        getComputedStyle: "readonly",
        ResizeObserver: "readonly",
        IntersectionObserver: "readonly",
        DOMParser: "readonly",
        XMLSerializer: "readonly",
      },
    },
  },

  // Vue 文件使用 TypeScript 解析器
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },

  // .d.ts 文件放宽规则
  {
    files: ["**/*.d.ts"],
    rules: {
      "@typescript-eslint/no-empty-object-type": "off",
      "@typescript-eslint/no-explicit-any": "off",
    },
  },

  // 项目自定义规则
  {
    rules: {
      // 放宽 TypeScript 严格规则，匹配现有代码风格
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],

      // Vue 规则调整
      "vue/multi-word-component-names": "off",
      "vue/no-v-html": "off",
      "vue/attributes-order": "warn",

      // 通用
      "no-console": ["warn", { allow: ["warn", "error"] }],
    },
  },

  // Prettier 兼容（必须放最后）
  eslintConfigPrettier,
];

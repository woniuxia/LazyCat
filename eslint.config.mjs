import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import eslintConfigPrettier from "eslint-config-prettier";
import globals from "globals";

export default [
  // 全局忽略
  {
    ignores: [
      "**/dist/**",
      "**/dist-renderer/**",
      "**/dist-ts/**",
      "**/node_modules/**",
      ".worktrees/**",
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

  // 桌面渲染层运行在 WebView 中
  {
    files: ["apps/desktop/**/*.{ts,vue}"],
    languageOptions: {
      globals: globals.browser,
    },
  },

  // 仓库脚本和构建配置运行在 Node.js 中
  {
    files: ["scripts/**/*.{js,mjs,cjs}", "*.config.{js,mjs,cjs,ts}"],
    languageOptions: {
      globals: {
        ...globals.node,
        // Puppeteer page.evaluate 回调在浏览器上下文执行
        document: "readonly",
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
      "vue/no-mutating-props": ["error", { shallowOnly: true }],
      "vue/attributes-order": "warn",

      // 通用
      "no-console": ["warn", { allow: ["warn", "error"] }],
    },
  },

  // Prettier 兼容（必须放最后）
  eslintConfigPrettier,
];

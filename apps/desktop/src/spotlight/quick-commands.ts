import type { QuickCommandDescriptor } from "./types";

export const QUICK_COMMAND_DESCRIPTORS: QuickCommandDescriptor[] = [
  {
    id: "todo-create",
    name: "新建任务",
    description: '以 "+ " 前缀快速创建一个待办事项',
    trigger: { type: "prefix", value: "+ " },
    defaultEnabled: true,
  },
  {
    id: "calc",
    name: "计算器",
    description: '以 "calc " 前缀计算表达式并复制结果',
    trigger: { type: "keyword", value: "calc" },
    defaultEnabled: true,
  },
];

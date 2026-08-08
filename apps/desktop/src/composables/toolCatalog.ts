import type { SidebarItem, ToolDef } from "../types";

export const HOME_ID = "home";
export const HOME_TOOL: ToolDef = {
  id: HOME_ID,
  name: "首页",
  desc: "收藏工具与近一个月高频工具入口",
};

const SIDEBAR_ITEMS: SidebarItem[] = [
  {
    kind: "tool",
    tool: { id: "formatter", name: "代码格式化", desc: "JSON/XML/HTML/Java/SQL 自动格式化" },
  },
  { kind: "tool", tool: { id: "calc-draft", name: "计算草稿", desc: "草稿式计算，保留历史记录" } },
  { kind: "tool", tool: { id: "regex", name: "正则工具", desc: "表达式生成与测试" } },
  { kind: "tool", tool: { id: "diff", name: "文本对比", desc: "双栏文本差异对比" } },
  { kind: "tool", tool: { id: "markdown", name: "Markdown", desc: "Markdown 编辑与实时预览" } },
  {
    kind: "group",
    group: {
      id: "more",
      name: "更多工具",
      tools: [
        { id: "snippets", name: "代码片段", desc: "代码片段收藏与管理" },
        { id: "launcher", name: "快捷启动", desc: "常用程序快速启动与管理" },
        { id: "browser-profiles", name: "浏览器身份", desc: "一键启动 Edge / Chrome 用户身份窗口" },
        { id: "todo", name: "任务清单", desc: "任务与周期事件管理" },
        { id: "pomodoro", name: "番茄钟", desc: "工作日自动启动的专注计时" },
        { id: "pm", name: "项目管理", desc: "项目工作项跟踪与看板" },
        { id: "weekly-work", name: "本周工作", desc: "按本周时间范围汇总工作项" },
        { id: "inbox", name: "收纳箱", desc: "后台剪贴板收件箱与历史整理" },
        { id: "widget", name: "桌面挂件", desc: "在桌面挂件中显示今日仪表盘" },
        { id: "release-package", name: "上线包打包", desc: "按项目构建前后端并归档上线产物" },
        { id: "action-center", name: "动作中心", desc: "组合并一键运行常用开发动作" },
        {
          id: "test-email-assistant",
          name: "测试邮件助手",
          desc: "从 Word 模板生成测试报告并整理邮件正文",
        },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "encode",
      name: "编解码",
      tools: [
        { id: "base64", name: "Base64", desc: "Base64 编码与解码" },
        { id: "url", name: "URL 编解码", desc: "URL Encode / Decode" },
        { id: "md5", name: "MD5", desc: "计算 MD5 摘要" },
        { id: "hash", name: "SHA/HMAC", desc: "SHA-1/256/512 与 HMAC-SHA256" },
        { id: "qr", name: "二维码生成", desc: "根据文本生成二维码" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "crypto",
      name: "加密与安全",
      tools: [
        { id: "rsa", name: "RSA 加解密", desc: "RSA 公私钥加解密" },
        { id: "aes", name: "AES/DES", desc: "AES / DES / 3DES 加解密" },
        { id: "jwt", name: "JWT 解析", desc: "离线解析 JWT Token" },
        { id: "uuid", name: "UUID/GUID", desc: "UUID 与 GUID 生成" },
        { id: "password", name: "密码工具", desc: "随机密码生成与强度分析" },
        { id: "bcrypt", name: "Bcrypt", desc: "Bcrypt 哈希生成与验证" },
        { id: "vault", name: "密码管理", desc: "应用/服务器/数据库密码加密存储" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "text",
      name: "数据转换",
      tools: [
        {
          id: "json-workbench",
          name: "JSON 工作台",
          desc: "JSON 处理与转换、Schema 校验和样例生成",
        },
        {
          id: "data-convert",
          name: "数据格式转换",
          desc: "CSV、JavaBean 与配置文件格式转换",
        },
        { id: "data-dictionary", name: "数据字典", desc: "JSON 数组字段释义与数据检索" },
        { id: "mybatis-helper", name: "MyBatis 助手", desc: "动态 SQL 渲染与占位符展开" },
        { id: "maven", name: "Maven 定位", desc: "本地 Maven 仓库 Jar 包定位与版本查询" },
        { id: "base-converter", name: "进制转换", desc: "二/八/十/十六进制转换" },
        { id: "color", name: "颜色转换", desc: "颜色格式互转与对比度检查" },
        { id: "escape-unescape", name: "转义/反转义", desc: "JSON/HTML/SQL/JS 字符串转义与反转义" },
        { id: "text-process", name: "文本处理", desc: "文本清洗、过滤提取与结果统计" },
        {
          id: "exception-stack",
          name: "异常堆栈整理器",
          desc: "JavaScript/TypeScript 与 Java 异常堆栈摘要",
        },
        { id: "naming-case", name: "命名转换", desc: "camelCase/snake_case/PascalCase 互转" },
        { id: "sql-entity", name: "SQL 转实体类", desc: "CREATE TABLE 转 Java/TS/Go/Python 实体" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "network",
      name: "网络与系统",
      tools: [
        {
          id: "network",
          name: "访问链路诊断",
          desc: "代理、Hosts、DNS、TCP、TLS、HTTP 分层诊断",
        },
        {
          id: "request-forward",
          name: "请求转发",
          desc: "HTTP、WebSocket、TCP 与 UDP 本地端口转发",
        },
        { id: "api-mock", name: "API Mock", desc: "本地接口 Mock 服务与路由管理" },
        { id: "dns", name: "DNS 查询", desc: "域名解析与记录查询" },
        { id: "hosts", name: "Hosts 管理", desc: "多配置保存与切换" },
        { id: "ports", name: "端口占用", desc: "端口占用与进程分析" },
        { id: "file-lock", name: "文件占用", desc: "定位 Windows 文件关联进程" },
        { id: "env", name: "环境检测", desc: "检测 Node 与 Java 版本" },
        { id: "nginx-helper", name: "Nginx 助手", desc: "静态站点 + API 反代配置生成与校验" },
        { id: "hotkey", name: "快捷键检测", desc: "全局快捷键冲突检测" },
        { id: "http-status", name: "HTTP 状态码", desc: "HTTP 状态码速查与说明" },
        { id: "chmod-calc", name: "chmod 计算器", desc: "Linux 文件权限数字/符号互转" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "files",
      name: "文件与媒体",
      tools: [
        { id: "split-merge", name: "切分与合并", desc: "大文件切片与合并" },
        { id: "pdf", name: "PDF 工具", desc: "PDF 合并、拆分与信息查看" },
        { id: "image", name: "图片转换", desc: "格式转换、缩放、裁剪、压缩" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "calc",
      name: "时间工具",
      tools: [
        { id: "timestamp", name: "时间戳转换", desc: "时间戳与日期互转" },
        { id: "cron", name: "Cron 工具", desc: "Cron 表达式生成与预览" },
        { id: "date-calc", name: "日期计算器", desc: "日期间隔与日期加减计算" },
      ],
    },
  },
  {
    kind: "group",
    group: {
      id: "manuals",
      name: "离线手册",
      tools: [
        { id: "manual-vue3", name: "Vue 3 手册", desc: "Vue 3 中文开发手册" },
        { id: "manual-element-plus", name: "Element Plus", desc: "Element Plus 组件文档" },
        { id: "manual-mdn-js", name: "JavaScript", desc: "MDN JavaScript 中文参考手册" },
      ],
    },
  },
];

const ALL_TOOLS: ToolDef[] = SIDEBAR_ITEMS.flatMap((item) =>
  item.kind === "group" ? item.group.tools : [item.tool],
);
const ALL_TOOL_MAP = new Map(ALL_TOOLS.map((tool) => [tool.id, tool]));

export function getSidebarItems(): SidebarItem[] {
  return SIDEBAR_ITEMS;
}

export function getAllTools(): ToolDef[] {
  return ALL_TOOLS;
}

export function getAllToolMap(): Map<string, ToolDef> {
  return ALL_TOOL_MAP;
}

export function isRealToolId(id: string): boolean {
  return ALL_TOOL_MAP.has(id);
}

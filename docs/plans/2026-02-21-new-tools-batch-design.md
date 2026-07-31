# 新工具批量设计文档

日期: 2026-02-21

## 概览

为 LazyCat 新增 10 个开发者工具，分 4 个批次实现。其中 5 个为新面板，5 个融入现有面板。

## 工具归组总表

| 工具             | 归入分组 | 侧边栏 ID        | 类型                  | 批次 |
| ---------------- | -------- | ---------------- | --------------------- | ---- |
| 命名风格转换     | 文本处理 | `naming-case`    | 新面板                | 1    |
| 文本统计         | —        | —                | 融入 TextProcessPanel | 1    |
| 密码强度分析     | —        | —                | 融入 UuidGenPanel     | 1    |
| 配置文件互转     | 数据转换 | `config-convert` | 新面板                | 2    |
| YAML 校验/格式化 | —        | —                | 融入 JsonYamlPanel    | 2    |
| JSON 压缩        | —        | —                | 融入 FormatterPanel   | 2    |
| HTTP 状态码速查  | 网络     | `http-status`    | 新面板                | 3    |
| chmod 计算器     | 网络     | `chmod-calc`     | 新面板                | 3    |
| 日期计算器       | 时间     | `date-calc`      | 新面板                | 4    |
| Bcrypt 哈希      | 加密     | `bcrypt`         | 新面板                | 4    |

## 实现方案

按分组批次推进（方案 A），每批完成后可独立验证。

## 前后端分工

统一走 Rust IPC，仅 JSON 压缩例外（前端 JSON.parse + JSON.stringify 即可）。

## 各工具详细设计

### 1. 命名风格转换 (NamingCasePanel)

- **侧边栏**: 文本处理组, id=`naming-case`, name="命名转换", desc="camelCase/snake_case/PascalCase 互转"
- **Rust 域**: `text`, action=`naming_convert`
- **IPC 通道**: `tool:text:naming-convert`
- **UI**: 上方输入框 + 下方 6 个只读结果框 (camelCase / PascalCase / snake_case / SCREAMING_SNAKE / kebab-case / dot.case)
- **逻辑**: 先按 `_`、`-`、`.`、空格、大小写边界拆分单词，再按目标风格拼接
- **批量**: 支持每行一个标识符，逐行转换
- **Rust payload**: `{ "input": "string" }`
- **Rust response**: `{ "camelCase": "...", "pascalCase": "...", "snakeCase": "...", "screamingSnake": "...", "kebabCase": "...", "dotCase": "..." }`

### 2. 文本统计（融入 TextProcessPanel）

- **改动范围**: TextProcessPanel.vue + text.rs
- **新增指标**: 字符数(含空格)、字符数(不含空格)、中文字数、英文单词数、字节数(UTF-8)、最长行长度
- **Rust 端**: 扩展 `process` action 返回的 `stats` 对象，新增上述字段
- **前端**: 摘要栏从 `repeat(4, ...)` 改为 `auto-fill, minmax(120px, 1fr)` 自适应网格
- **无新 IPC 通道**: 复用现有 `tool:text:process`

### 3. 密码强度分析（融入 UuidGenPanel）

- **改动范围**: UuidGenPanel.vue + gen.rs
- **Rust 域**: `gen`, action=`password_strength`
- **IPC 通道**: `tool:gen:password-strength`
- **UI**: 在密码生成器区域下方增加「强度分析」卡片，含强度等级、进度条、扣分项说明
- **评分维度**: 长度、大小写混合、数字、特殊字符、连续重复、常见密码字典匹配
- **联动**: 生成密码时自动填入分析框
- **Rust payload**: `{ "password": "string" }`
- **Rust response**: `{ "score": 0-100, "level": "weak|medium|strong|very_strong", "details": [{ "rule": "...", "passed": bool, "message": "..." }] }`

### 4. 配置文件互转 (ConfigConvertPanel)

- **侧边栏**: 数据转换组, id=`config-convert`, name="配置互转", desc="Properties/YAML/TOML/.env 格式互转"
- **Rust 域**: `convert`, action=`config_convert`
- **IPC 通道**: `tool:convert:config-convert`
- **UI**: 左右双栏编辑器 + 顶部源/目标格式下拉框 (Properties / YAML / TOML / .env)
- **转换策略**: 所有格式先解析为 serde_json::Value 中间表示，再序列化为目标格式
- **Rust 新增依赖**: `toml` crate; Properties 和 .env 用自定义解析
- **Rust payload**: `{ "input": "string", "from": "properties|yaml|toml|env", "to": "properties|yaml|toml|env" }`
- **Rust response**: `{ "output": "string" }`

### 5. YAML 校验/格式化（融入 JsonYamlPanel）

- **改动范围**: JsonYamlPanel.vue (或对应面板) + convert.rs
- **Rust 域**: `convert`, 新增 action=`yaml_validate` 和 `yaml_format`
- **IPC 通道**: `tool:convert:yaml-validate`, `tool:convert:yaml-format`
- **UI**: 在现有 JSON↔YAML 面板中增加「YAML 校验」和「YAML 格式化」按钮
- **校验**: 检查语法错误并返回错误行号和描述
- **格式化**: 统一缩进为 2 空格
- **Rust 依赖**: 复用已有 `serde_yaml`
- **yaml_validate payload**: `{ "input": "string" }`
- **yaml_validate response**: `{ "valid": bool, "error": { "line": number, "message": "string" } | null }`
- **yaml_format payload**: `{ "input": "string", "indent": 2 }`
- **yaml_format response**: `{ "output": "string" }`

### 6. JSON 压缩（融入 FormatterPanel）

- **改动范围**: FormatterPanel.vue 仅前端
- **无 Rust IPC**: 纯前端 `JSON.parse()` → `JSON.stringify()` 无缩进
- **UI**: 当语言选择 JSON 时，在操作按钮区增加「压缩」按钮
- **错误处理**: JSON 解析失败时显示错误提示

### 7. HTTP 状态码速查 (HttpStatusPanel)

- **侧边栏**: 网络组, id=`http-status`, name="HTTP 状态码", desc="HTTP 状态码速查与说明"
- **Rust 域**: `network`, action=`http_status_lookup` 和 `http_status_list`
- **IPC 通道**: `tool:network:http-status-lookup`, `tool:network:http-status-list`
- **UI**: 顶部搜索框 (按码或描述搜索) + 下方按分类 (1xx/2xx/3xx/4xx/5xx) 折叠展示
- **数据**: 纯静态，内嵌在 Rust 端，每个状态码含: 码值、英文名、中文说明、常见使用场景
- **http_status_list response**: `{ "groups": [{ "category": "1xx", "name": "信息响应", "codes": [...] }] }`
- **http_status_lookup payload**: `{ "query": "string" }`
- **http_status_lookup response**: `{ "results": [{ "code": 200, "name": "OK", "desc": "...", "usage": "..." }] }`

### 8. chmod 计算器 (ChmodCalcPanel)

- **侧边栏**: 网络组, id=`chmod-calc`, name="chmod 计算器", desc="Linux 文件权限数字/符号互转"
- **Rust 域**: `network`, action=`chmod_calc`
- **IPC 通道**: `tool:network:chmod-calc`
- **UI**: 上半部分 3x3 复选框矩阵 (Owner/Group/Other x Read/Write/Execute)，下半部分显示数字 (如 `755`) 和符号 (如 `rwxr-xr-x`)
- **双向联动**: 改复选框更新数字，改数字更新复选框
- **快捷按钮**: 644、755、777、600
- **Rust payload**: `{ "mode": "numeric|symbolic", "value": "string" }`
- **Rust response**: `{ "numeric": "755", "symbolic": "rwxr-xr-x", "owner": { "read": true, "write": true, "execute": true }, "group": { ... }, "other": { ... } }`

### 9. 日期计算器 (DateCalcPanel)

- **侧边栏**: 时间组, id=`date-calc`, name="日期计算器", desc="日期间隔计算与日期加减"
- **Rust 域**: `time`, action=`date_diff` 和 `date_add`
- **IPC 通道**: `tool:time:date-diff`, `tool:time:date-add`
- **UI 两个功能区**:
  - 日期间隔: 两个日期选择器 → 相差天数/小时/分钟/秒 + 自然语言 (X年X月X天)
  - 日期加减: 一个日期 + 输入天数/小时/分钟 → 结果日期
- **Rust 依赖**: 复用已有 `chrono` crate
- **date_diff payload**: `{ "start": "2026-01-01", "end": "2026-02-21" }`
- **date_diff response**: `{ "days": 51, "hours": 1224, "minutes": 73440, "seconds": 4406400, "natural": "0年1月21天" }`
- **date_add payload**: `{ "date": "2026-02-21", "add": { "days": 30, "hours": 0, "minutes": 0 } }`
- **date_add response**: `{ "result": "2026-03-23", "resultDatetime": "2026-03-23T00:00:00" }`

### 10. Bcrypt 哈希 (BcryptPanel)

- **侧边栏**: 加密组, id=`bcrypt`, name="Bcrypt", desc="Bcrypt 哈希生成与验证"
- **Rust 域**: `crypto`, action=`bcrypt_hash` 和 `bcrypt_verify`
- **IPC 通道**: `tool:crypto:bcrypt-hash`, `tool:crypto:bcrypt-verify`
- **UI 两个功能区**:
  - 生成: 输入明文 + cost 因子 (默认 12, 范围 4-31) → 生成 bcrypt 哈希
  - 验证: 输入明文 + 哈希值 → 匹配/不匹配
- **Rust 新增依赖**: `bcrypt` crate
- **bcrypt_hash payload**: `{ "password": "string", "cost": 12 }`
- **bcrypt_hash response**: `{ "hash": "$2b$12$..." }`
- **bcrypt_verify payload**: `{ "password": "string", "hash": "$2b$12$..." }`
- **bcrypt_verify response**: `{ "valid": true }`

## 批次计划

### 批次 1: 文本组 + 密码强度

- 命名风格转换 (新面板 NamingCasePanel + text.rs 新 action)
- 文本统计 (扩展 TextProcessPanel + text.rs stats 扩展)
- 密码强度分析 (扩展 UuidGenPanel + gen.rs 新 action)

### 批次 2: 数据转换组

- 配置文件互转 (新面板 ConfigConvertPanel + convert.rs 新 action + toml crate)
- YAML 校验/格式化 (扩展 JsonYamlPanel + convert.rs 新 action)
- JSON 压缩 (扩展 FormatterPanel, 纯前端)

### 批次 3: 网络组

- HTTP 状态码速查 (新面板 HttpStatusPanel + network.rs 新 action)
- chmod 计算器 (新面板 ChmodCalcPanel + network.rs 新 action)

### 批次 4: 时间组 + 加密组

- 日期计算器 (新面板 DateCalcPanel + time.rs 新 action)
- Bcrypt 哈希 (新面板 BcryptPanel + crypto.rs 新 action + bcrypt crate)

## 新增 Rust 依赖

| crate    | 用途                          | 批次 |
| -------- | ----------------------------- | ---- |
| `toml`   | 配置文件互转 TOML 解析/序列化 | 2    |
| `bcrypt` | Bcrypt 哈希生成与验证         | 4    |

## 每个新面板的接入清单

1. 创建 `apps/desktop/src/components/<Name>Panel.vue`
2. 在 `apps/desktop/src/tool-registry.ts` 注册组件映射
3. 在 `apps/desktop/src/App.vue` 的 `sidebarItems` 对应分组中添加工具定义
4. 在 `apps/desktop/src/bridge/tauri.ts` 的 `CHANNEL_MAP` 添加 IPC 通道映射
5. 在 `apps/desktop/src-tauri/src/tools/<domain>.rs` 添加 action 处理
6. 在 `apps/desktop/src-tauri/src/tools/mod.rs` 注册新模块 (如有新域)

## 每个融入面板的接入清单

1. 修改现有 Vue 组件，增加 UI 区域
2. 在 `CHANNEL_MAP` 添加新 IPC 通道 (如需要)
3. 在对应 Rust 域模块添加新 action (如需要)

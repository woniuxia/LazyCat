# 项目管理接入思源目录预览 V1 设计

日期：2026-03-29

> 目标：在项目管理面板内新增思源设置入口，支持配置本地思源服务地址与 API Token，完成连接验证，并预览思源的笔记本与文档目录树。第一版只打通配置、验证、目录读取链路，不实现项目绑定、自动同步或写入能力。

## 1. 背景与目标

### 1.1 当前现状

- 项目管理功能集中在 `apps/desktop/src/components/PmPanel.vue` 与 `apps/desktop/src-tauri/src/tools/pm.rs`。
- 全局轻量配置统一通过 `user_settings` 表存储，前端读写封装在 `apps/desktop/src/composables/useSettings.ts`。
- 当前仓库中尚无思源接入，也没有目录树预览能力。

### 1.2 本轮目标

1. 在项目管理面板中提供思源设置入口。
2. 支持保存思源 `baseUrl` 与 `token`。
3. 支持测试连接，确认地址、Token 和本地思源服务可用。
4. 支持预览思源中的笔记本与文档目录树。
5. 配置在应用重启后仍可保留。

### 1.3 非目标

1. 不实现项目与思源目录/文档的绑定关系。
2. 不实现目录缓存表或离线同步。
3. 不实现思源文档写入、创建、更新、删除。
4. 不把设置入口扩展到全局设置页。
5. 不在本轮引入加密存储 Token 的新机制。

## 2. 已确认决策

1. 设置入口放在项目管理面板内部，而不是全局设置页。
2. 第一版只做“连接验证 + 目录树预览”，不做绑定逻辑。
3. 目录预览展示“笔记本 + 文档树”。
4. 配置继续复用 `user_settings`，不新增专门数据表。
5. 思源调用从 Rust 侧发起，不在前端直接请求。

## 3. 交互设计

### 3.1 入口

- 在 `PmPanel.vue` 顶部工具栏右侧新增 `思源设置` 按钮。
- 点击后打开 `el-drawer`，不打断当前项目管理主视图。

### 3.2 抽屉内容分区

1. **连接配置**
   - 服务地址输入框
   - API Token 输入框
   - Token 默认密码态，支持显示/隐藏
2. **操作区**
   - `保存配置`
   - `测试连接`
   - `加载目录`
3. **结果区**
   - 显示连接状态与思源版本
   - 显示笔记本与文档树
   - 失败时显示明确错误信息

### 3.3 交互行为

- `保存配置`：仅保存地址和 Token，不自动请求思源。
- `测试连接`：调用思源版本接口验证配置可用。
- `加载目录`：拉取笔记本和目录树并更新预览区。
- 若此前已有成功目录，本次刷新失败时保留旧目录显示。

## 4. 数据与存储设计

### 4.1 `user_settings` key

- `pm_siyuan_base_url`
- `pm_siyuan_token`

### 4.2 前端类型

新增 PM 思源相关类型，建议放在 `apps/desktop/src/types/pm.ts`：

```ts
export interface PmSiyuanConfig {
  baseUrl: string;
  token: string;
}

export interface PmSiyuanTreeNode {
  id: string;
  name: string;
  hpath: string;
  path: string | null;
  leaf: boolean;
  children: PmSiyuanTreeNode[];
}

export interface PmSiyuanNotebookDirectory {
  id: string;
  name: string;
  icon: string | null;
  closed: boolean;
  docCount: number;
  children: PmSiyuanTreeNode[];
}

export interface PmSiyuanDirectoryResult {
  notebooks: PmSiyuanNotebookDirectory[];
  fetchedAt: string;
}
```

### 4.3 前端状态

在 `PmPanel.vue` 中新增独立状态：

- `siyuanDrawerVisible`
- `siyuanForm`
- `siyuanShowToken`
- `siyuanTesting`
- `siyuanLoadingDirectory`
- `siyuanConnected`
- `siyuanVersion`
- `siyuanDirectory`
- `siyuanError`

## 5. 接口设计

### 5.1 前端通道

在 `apps/desktop/src/bridge/tauri.ts` 中新增：

- `tool:pm:siyuan-test`
- `tool:pm:siyuan-directory`

### 5.2 Rust `pm` 域 action

在 `apps/desktop/src-tauri/src/tools/pm.rs` 中新增：

- `siyuan_test`
- `siyuan_directory`

### 5.3 思源官方接口

已确认使用的官方接口：

1. `POST /api/system/version`
   - 用于连接验证
2. `POST /api/notebook/lsNotebooks`
   - 用于笔记本列表
3. `POST /api/query/sql`
   - 用于查询文档记录并在本地构树

所有请求均为：

- `Content-Type: application/json`
- `Authorization: Token xxx`

标准响应格式：

```json
{
  "code": 0,
  "msg": "",
  "data": {}
}
```

## 6. 目录树构建方案

### 6.1 读取流程

1. 调 `lsNotebooks` 获取笔记本元数据。
2. 调 `query/sql` 查询文档记录。
3. 按笔记本分组。
4. 根据 `hpath` 构建文档树。
5. 返回前端最终树结构。

### 6.2 SQL 方案

第一版拟采用如下查询：

```sql
SELECT id, box, path, hpath, content, sort
FROM blocks
WHERE type = 'd'
ORDER BY box ASC, hpath ASC, sort ASC, id ASC
```

说明：

- `lsNotebooks` 与 `query/sql` 为 `API.md` 明确文档化接口。
- `blocks` 表中的 `box / path / hpath / content / sort` 字段用于构树，属于基于思源当前实现的工程性假设。
- 若用户本地思源版本与该字段结构不兼容，本轮直接给出明确错误，不做静默降级。

### 6.3 构树规则

- `box` 视为笔记本 ID。
- `hpath` 形如 `/A/B/C`。
- 以 `/` 分段，在 Rust 内存中递归/迭代构造树。
- 每个叶子节点保留真实文档 `id`、`name`、`hpath`、`path`。
- 中间目录节点若无真实文档 ID，可使用稳定派生 ID，如 `box:hpath`。

## 7. 错误处理

### 7.1 地址与 Token 校验

- 地址必须为 `http://` 或 `https://` 开头。
- 地址保存前去除末尾 `/`。
- Token 不允许为空。

### 7.2 错误分类

- 连接失败：`无法连接到思源服务，请检查地址和本地服务状态`
- 鉴权失败：`思源鉴权失败，请检查 API Token`
- 思源业务错误：优先透传 `msg`
- SQL/目录构建不兼容：`当前思源版本暂不支持该目录读取方式`

### 7.3 超时

- 测试连接：5 秒
- 加载目录：10 秒

## 8. 安全与权衡

- 第一版 Token 按现有 `user_settings` 模式明文存储。
- 这是功能优先的阶段性方案，后续再评估是否接入密码库或系统凭据管理。
- 前端不直接访问思源，避免浏览器侧跨域和 Token 暴露细节。

## 9. 验收标准

1. PM 面板中能打开思源设置抽屉。
2. 地址与 Token 可保存并在重启后恢复。
3. 测试连接可正确显示成功或失败。
4. 成功时可显示思源版本。
5. 可加载笔记本与文档目录树。
6. 地址错误、Token 错误、思源未启动时有明确提示。

## 10. 验证计划

- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`

优先补充的测试：

- Rust 侧 URL 归一化
- 思源标准响应解析
- 目录树构建

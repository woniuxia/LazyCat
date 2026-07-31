# 请求转发工具设计

## 概述

新增独立的“请求转发”工具，用于在 LazyCat 内创建并同时运行多条本地监听规则，将 HTTP、TCP 或 UDP 流量一对一转发到固定下游。

HTTP 使用应用层代理语义，理解请求与响应并提供结构化日志；TCP 和 UDP 使用通用字节流/数据报转发。规则持久化保存，正在运行的规则在应用下次启动时自动恢复。整个功能离线运行，不依赖外部代理程序或公网资源。

## 目标

1. 支持多条独立规则并发运行，每条规则配置协议、本地监听端点和固定下游。
2. 支持 HTTP -> HTTP/HTTPS，透传 Method、Path、Query、Headers 和 Body。
3. 支持 TCP 双向字节流转发和 UDP 双向数据报转发。
4. 默认仅监听 `127.0.0.1`，允许用户显式选择局域网地址或 `0.0.0.0`。
5. 持久化规则、累计统计和有界日志，并自动恢复上次仍在运行的规则。
6. 明确区分期望运行状态与实际运行状态，任何启动失败都显式展示。
7. 支持单条启停、全部启停、停止后编辑，以及按规则查看状态和日志。

## 非目标

1. 不支持单条规则配置多个下游、负载均衡或故障转移。
2. 不支持按 Host、Path 或其他条件路由到不同下游。
3. 不支持本地 HTTPS 监听、证书管理或 TLS 中间人代理。
4. 不支持 WebSocket Upgrade；支持普通 HTTP、流式响应和 SSE。
5. 不支持请求/响应改写，不提供 Path、Header 或 Body 规则系统。
6. 不提供 HTTP/TCP/UDP 原始全量抓包能力。
7. 不内置访问认证、用户权限、远程控制或云同步。
8. 不引入独立代理进程或外部配置文件作为运行时真值。

## 用户流程

### 创建并启动规则

1. 用户进入独立“请求转发”工具并新建规则。
2. 选择 HTTP、TCP 或 UDP，填写名称、本地监听地址/端口和固定下游。
3. HTTP 下游填写 `http://` 或 `https://` Base URL；TCP/UDP 填写 Host 和 Port。
4. 用户保存规则，选择“仅保存”或“保存并启动”。
5. 后端校验配置并尝试绑定监听地址。
6. 启动成功后实际状态显示“运行中”，规则被标记为下次启动自动恢复。
7. 启动失败时显示具体原因，其他规则不受影响。

### 停止并编辑规则

1. 运行中的规则配置只读，仍可查看统计和日志。
2. 用户点击“停止并编辑”。
3. 后端停止监听并取消该规则的存量任务，规则变为“已停止”。
4. 停止状态会关闭下次启动自动恢复，但不删除配置、累计统计或历史日志。
5. 用户可修改全部配置，并选择“仅保存”或“保存并启动”。

### 应用启动自动恢复

1. 数据库初始化完成后读取标记为自动恢复的规则。
2. 运行管理器逐条启动，规则之间互不阻塞。
3. 端口冲突、地址无效等问题只让对应规则进入“启动失败”。
4. 前端打开面板时合并数据库配置与运行管理器状态，不根据数据库字段伪造“运行中”。

## 总体架构

采用单一 Tokio 后台引擎，不沿用 API Mock 的每连接标准线程模型，也不管理外部代理进程。

```text
本地客户端
    |
    v
HTTP / TCP / UDP 协议处理器
    |
    v
固定 HTTP(S) / TCP / UDP 下游

SQLite 规则仓库 <-> 运行管理器 <-> 状态 / 统计 / 日志
```

### 前端面板

职责：

1. 管理规则表单和选中态。
2. 调用规则 CRUD、启停、状态、统计和日志 action。
3. 展示实际运行状态、错误、流量和 HTTP 详情日志。
4. 在敏感监听地址、删除、清日志和停止存量连接前给出明确提示。

前端不持有服务运行真值，也不自行推断自动恢复结果。

### `request_forward` 工具域

新增独立 Rust 工具域，负责：

1. 规则 CRUD 和数据库校验。
2. 运行管理器 action 编排。
3. 日志、统计查询和清理。
4. 启动时规则恢复入口。

该域通过现有 `tool_execute` / `CHANNEL_MAP` 分发接入，不新增独立 Tauri command。

### 运行管理器

进程内实际运行状态的唯一真值，按规则 ID 管理运行实例。

每个运行实例至少包含：

1. 协议类型与已冻结的规则配置快照。
2. 取消令牌或等价的显式停止信号。
3. 监听任务句柄和子任务集合。
4. 当前状态、启动时间和最后错误。
5. 并发安全的实时统计计数器。

运行管理器接口保持协议无关：启动、停止、停止全部、查询状态。协议差异封装在独立处理器中。

### 协议处理器

HTTP、TCP、UDP 处理器各自只负责协议行为，依赖统一的取消、限制、统计和日志接口。一个处理器的故障不得停止其他规则。

## 规则模型

共同字段：

```ts
type RequestForwardProtocol = "http" | "tcp" | "udp";

interface RequestForwardRule {
  id: number;
  name: string;
  protocol: RequestForwardProtocol;
  bindHost: string;
  listenPort: number;
  targetUrl: string | null;
  targetHost: string | null;
  targetPort: number | null;
  captureHttpHeaders: boolean;
  captureHttpBody: boolean;
  autoStart: boolean;
  createdAt: string;
  updatedAt: string;
}
```

规则：

1. HTTP 使用 `targetUrl`，必须为 `http://` 或 `https://`，TCP/UDP 使用 `targetHost + targetPort`。
2. 默认 `bindHost = 127.0.0.1`。
3. `listenPort` 和 `targetPort` 范围为 `1..=65535`。
4. `bindHost` 第一版只接受 IP 字面量，不接受主机名；支持 IPv4 和 IPv6，UI 展示 IPv6 端点时使用 `[addr]:port`。
5. 协议保存后不可直接切换；用户需要创建新规则，避免协议专属字段残留。
6. `autoStart` 表示下次应用启动时应尝试恢复，不代表当前实际已运行；它由后端运行 action 管理，`create/update` payload 不接受客户端直接修改。
7. 运行中的规则配置只读；必须先停止再更新。
8. 后端对所有规则重复校验，不能依赖前端表单约束。
9. 启动前解析下游地址并执行自转发检查：若下游端口等于本规则监听端口，且下游解析地址会命中本规则监听范围，则拒绝启动。`0.0.0.0` / `::` 视为覆盖本机对应地址族；允许显式转发到其他规则的不同监听端口。

## HTTP 转发

HTTP 使用 Hyper/Hyper-Util 一类的异步服务与客户端能力，并使用 Rustls 系 TLS 客户端支持 HTTPS 下游。具体 crate 版本在实施计划中按当前 Tauri/Tokio 依赖兼容性确定。

数据流：

1. 接收本地 HTTP 请求。
2. 将请求 Path 和 Query 合并到规则的下游 Base URL。Base URL 允许可选路径前缀，但禁止 Query 和 Fragment；保存时移除尾部 `/` 作为规范化基线。入站 Path 保留前导 `/`，最终 Path 为 `basePath + inboundPath`，例如 `https://a.example/api` + `/users` 得到 `/api/users`；入站 Query 原样作为最终 Query。
3. 保留 Method 和 Body 流。
4. 过滤逐跳 Header，例如 `Connection`、`Keep-Alive`、`Proxy-Authenticate`、`Proxy-Authorization`、`TE`、`Trailer`、`Transfer-Encoding`、`Upgrade`，以及 `Connection` Header 点名的字段。
5. 设置正确的下游 Host。删除客户端传入的 `Forwarded`、`X-Forwarded-For`、`X-Forwarded-Host`、`X-Forwarded-Proto` 后由代理重建：`for`/`X-Forwarded-For` 使用直接客户端 IP，`host`/`X-Forwarded-Host` 使用原始 Host，`proto`/`X-Forwarded-Proto` 固定为本地监听协议 `http`。第一版不信任或追加客户端提供的代理链。
6. 将下游响应以流式方式返回客户端，并过滤响应逐跳 Header。
7. SSE 响应持续流式转发，不进行整包缓冲。

第一版不接受 WebSocket Upgrade。检测到 Upgrade 请求时返回明确的客户端错误，并记录“不支持 WebSocket”。

### HTTP 错误语义

1. 下游 DNS、连接或 TLS 建连失败返回 `502 Bad Gateway`。
2. 连接、请求或响应等待超时返回 `504 Gateway Timeout`。
3. 无效客户端请求由 HTTP 服务层返回对应 `4xx`。
4. 已经开始向客户端流式发送响应后发生下游错误时，只能终止响应流并记录错误，不伪造新的状态码。
5. 单个请求失败不停止监听规则。
6. 达到 HTTP 并发上限时不排队、不连接下游，立即返回 `503 Service Unavailable`，记录过载日志并增加错误计数。

## TCP 转发

1. 每接受一个客户端连接，创建独立异步连接任务。
2. 连接固定下游后使用 Tokio 双向复制传输字节。
3. 一侧读到 EOF 时正确执行对应写半关闭，允许另一方向继续完成剩余数据。
4. 下游连接失败时关闭当前客户端连接并写日志，不停止监听规则。
5. 停止规则时先停止接受新连接，再取消并关闭该规则现有连接。
6. 连接日志记录客户端、下游、开始/结束时间、上下行字节和错误，不保存原始负载。
7. 达到 TCP 并发上限时接受后立即关闭新连接，记录过载日志并增加错误计数；现有连接不受影响。

## UDP 转发

UDP 无连接，必须按客户端地址隔离下游响应路径。

1. 监听 socket 接收客户端数据报。
2. 以客户端 SocketAddr 为键创建或复用临时会话。
3. 每个会话持有独立的已连接下游 UDP socket，数据报原样发往固定下游。
4. 下游响应通过该会话回送到原客户端，不能让多个客户端共享一个无法判定响应归属的 socket。
5. 会话按最后活动时间回收；固定空闲超时与最大会话数由实现常量控制。
6. 达到会话上限时拒绝创建新会话并记录明确错误，现有会话继续工作。
7. 停止规则时关闭监听与全部会话。
8. UDP 日志记录客户端、数据报数量、上下行字节和错误，不保存原始负载。
9. 会话已存在时继续接收；只有新客户端达到会话上限时丢弃该数据报，并增加错误计数。

## 并发、取消和限制

第一版使用固定、保守的实现常量，不向用户暴露复杂调参界面：

1. 每条规则限制并发 HTTP 请求或 TCP 连接数。
2. UDP 限制活跃客户端会话数，并定期清理空闲会话。
3. HTTP 限制请求头大小和日志采集大小；转发正文采用流式处理，不因日志关闭而整包缓冲。
4. 网络连接和请求阶段设置明确超时；SSE 的已建立响应流不使用普通响应总时长超时。
5. 所有监听和连接任务响应规则级取消信号。
6. 停止 action 等待监听任务退出并完成必要清理，再返回成功；若清理异常则返回明确错误。
7. 过载不进入全局队列：HTTP 返回 503，TCP 关闭新连接，UDP 丢弃新客户端数据报；三者都写日志并增加错误计数。

具体默认数值应作为命名常量集中管理，并通过边界测试验证，不散落在协议实现中。

## 状态模型

实际状态：

```ts
type RequestForwardRuntimeState = "stopped" | "starting" | "running" | "stopping" | "failed";
```

规则：

1. `autoStart` 是持久化期望；`runtimeState` 只来自运行管理器。
2. `failed` 是终止态：监听器和全部子任务已经退出，规则不占用端口，可直接编辑、删除或重新启动。运行中发生可恢复的单请求/连接错误不进入 failed；只有规则级任务退出并完成清理后才能进入 failed。
3. 用户启动顺序为：运行管理器成功绑定并进入 running -> 持久化 `auto_start = true` -> action 成功。若持久化失败，必须停止刚启动的实例作为补偿；补偿也失败时返回包含两段原因的一致性错误，并以运行管理器实际状态为准展示，不能返回成功。
4. 用户停止顺序为：运行管理器完成停止并进入 stopped -> 持久化 `auto_start = false` -> action 成功。若持久化失败，尝试重新启动旧配置作为补偿；补偿也失败时返回一致性错误并展示实际状态，不能返回成功。
5. 应用启动恢复不修改 `auto_start`。恢复失败时保留 `auto_start = true`，以表达用户期望未改变；状态显示 failed，且本次进程内不无限自动重试。
6. 用户修改 failed 规则并选择“仅保存”时更新配置并将 `auto_start` 设为 false；选择“保存并启动”时重新尝试启动。
7. 相同规则的启停和更新操作按规则 ID 串行化，重复 start/stop 返回幂等结果或明确的状态冲突，不产生两个监听实例。

## 数据模型

新增三张表，不修改现有业务表。

### `request_forward_rules`

保存规则事实源和自动恢复期望：

```sql
CREATE TABLE IF NOT EXISTS request_forward_rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL,
  bind_host TEXT NOT NULL,
  listen_port INTEGER NOT NULL,
  target_url TEXT,
  target_host TEXT,
  target_port INTEGER,
  capture_http_headers INTEGER NOT NULL DEFAULT 1,
  capture_http_body INTEGER NOT NULL DEFAULT 0,
  auto_start INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

数据库不对端口做全局唯一约束，因为不同本地地址或 TCP/UDP 可以合法复用端口；实际可绑定性由运行时 socket bind 决定。

### `request_forward_stats`

每条规则一行累计统计：

```sql
CREATE TABLE IF NOT EXISTS request_forward_stats (
  rule_id INTEGER PRIMARY KEY,
  event_count INTEGER NOT NULL DEFAULT 0,
  upload_bytes INTEGER NOT NULL DEFAULT 0,
  download_bytes INTEGER NOT NULL DEFAULT 0,
  error_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(rule_id) REFERENCES request_forward_rules(id) ON DELETE CASCADE
);
```

实时计数先在内存中累加，再按受控频率或连接/请求结束时写入，避免每个数据块都写 SQLite。停止规则和应用正常退出时执行最终 flush。

`event_count` 使用协议相关但稳定的口径：HTTP 为已接收请求数，TCP 为已接受客户端连接数，UDP 为从客户端收到的数据报数。前端按协议显示“请求数 / 连接数 / 数据报数”，不能统一写成含糊的“连接或请求”。

### `request_forward_logs`

保存统一摘要与 HTTP 可选详情：

```sql
CREATE TABLE IF NOT EXISTS request_forward_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  rule_id INTEGER NOT NULL,
  protocol TEXT NOT NULL,
  client_addr TEXT,
  target_addr TEXT NOT NULL,
  method TEXT,
  path TEXT,
  status_code INTEGER,
  duration_ms INTEGER,
  upload_bytes INTEGER NOT NULL DEFAULT 0,
  download_bytes INTEGER NOT NULL DEFAULT 0,
  request_headers_json TEXT,
  response_headers_json TEXT,
  request_body_preview TEXT,
  response_body_preview TEXT,
  request_body_truncated INTEGER NOT NULL DEFAULT 0,
  response_body_truncated INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(rule_id) REFERENCES request_forward_rules(id) ON DELETE CASCADE
);
```

索引至少覆盖 `(rule_id, created_at DESC, id DESC)`。每条规则只保留最近 1000 条；插入新日志后清理超出部分。删除规则通过外键级联删除统计和日志。

## 日志与隐私

1. 所有协议默认记录状态、错误、端点、耗时和流量摘要。
2. HTTP Header 默认记录，但 `Authorization`、`Proxy-Authorization`、`Cookie` 和 `Set-Cookie` 的值保存为统一脱敏标记。
3. HTTP 正文采集默认关闭，按规则开启。
4. 正文只采集明确的文本或结构化 Content-Type，例如 text、JSON、XML、form-urlencoded。
5. 请求和响应正文分别最多记录前 64 KiB，超出设置截断标记；该限制只影响日志，不限制实际转发正文。
6. 二进制正文不保存预览，只记录 Content-Type 和字节数。
7. 存在非 identity `Content-Encoding` 的正文不尝试解压或保存预览，只记录编码、Content-Type 和字节数。
8. 流式正文采集使用旁路限长观察，不将完整请求或响应缓冲进内存。
9. 清空日志不重置累计统计；统计另有显式重置 action。

## 安全边界

1. 默认绑定 `127.0.0.1`。
2. 选择 `0.0.0.0`、局域网 IP 或其他非 loopback 地址时，前端显示该端口可能被局域网设备访问的提醒。
3. 第一版不内置访问认证；访问控制依赖绑定地址和 Windows 防火墙。
4. 不允许空目标、零端口、非 HTTP(S) URL 或结构不匹配的协议字段。
5. 下游地址允许 DNS 名称和 IP；DNS/连接错误显式进入请求或连接日志。
6. 不静默回退到其他监听地址、端口或下游。

## 前端设计

新增独立 `RequestForwardPanel.vue`，采用双栏布局：

### 左侧规则列表

1. 新建按钮和关键词搜索。
2. 每条规则展示名称、协议、本地端点、下游摘要和实际状态。
3. 状态包括运行中、已停止、启动中、停止中、启动失败。
4. 顶部提供“全部启动”和“全部停止”。批量操作逐条执行并汇总成功/失败，不因单条失败中断全部操作。

### 右侧配置和状态

1. 运行中配置只读，提供“停止并编辑”。
2. 已停止或失败规则可编辑，提供“仅保存”和“保存并启动”。
3. 显示请求/连接数、上下行字节和错误数。
4. HTTP 显示 Base URL 和日志采集选项；TCP/UDP 显示目标 Host/Port。
5. 非 loopback 监听显示风险提醒。
6. 删除前二次确认；运行中的规则必须先停止，删除操作不隐式吞掉停止失败。

### 日志区域

1. 默认显示当前规则最近日志。
2. 支持按成功/错误和关键词做基础筛选。
3. HTTP 日志可展开查看脱敏 Headers 与可选正文预览。
4. TCP/UDP 日志只显示连接/会话摘要和流量。
5. 关键词筛选匹配客户端地址、目标地址、HTTP Method/Path、状态码和错误文本；后端先应用筛选和稳定排序，再分页返回。
6. 清空当前规则日志前二次确认。

组件只负责编排；端点展示、表单校验参数构建、状态文案和日志格式化等适合复用的逻辑抽到 `src/utils/requestForward.ts` 并配套单测。

## IPC 设计

新增 `request_forward` domain，action 保持小而明确：

| Channel                            | Action        | 说明                                  |
| ---------------------------------- | ------------- | ------------------------------------- |
| `tool:request-forward:list`        | `list`        | 规则列表与实际状态摘要                |
| `tool:request-forward:get`         | `get`         | 单条规则详情                          |
| `tool:request-forward:create`      | `create`      | 创建停止状态规则                      |
| `tool:request-forward:update`      | `update`      | 更新已停止/失败规则，不接受 autoStart |
| `tool:request-forward:delete`      | `delete`      | 删除已停止或 failed 规则及关联数据    |
| `tool:request-forward:start`       | `start`       | 启动单条规则并设置自动恢复            |
| `tool:request-forward:stop`        | `stop`        | 停止单条规则并关闭自动恢复            |
| `tool:request-forward:start-all`   | `start_all`   | 启动全部未运行规则并返回逐条结果      |
| `tool:request-forward:stop-all`    | `stop_all`    | 停止全部运行规则并返回逐条结果        |
| `tool:request-forward:status`      | `status`      | 查询一条或全部实际状态                |
| `tool:request-forward:log-list`    | `log_list`    | 分页查询规则日志                      |
| `tool:request-forward:log-clear`   | `log_clear`   | 清空规则日志                          |
| `tool:request-forward:stats-get`   | `stats_get`   | 获取累计与实时统计                    |
| `tool:request-forward:stats-reset` | `stats_reset` | 显式重置累计统计                      |

`create` 不隐式启动，避免数据库写入成功但监听失败时产生含糊结果。前端“保存并启动”按 `create/update -> start` 顺序执行，分别反馈保存错误与启动错误。

## 应用生命周期

1. 主应用初始化数据库和工具状态后创建共享转发 runtime/manager。
2. 自动恢复不能阻塞主窗口展示；恢复任务在后台执行并保存每条规则结果。
3. 应用退出时先发出全局取消，停止监听和连接任务，再 flush 统计。
4. 用户从面板停止规则会更新 `auto_start = false`；应用正常退出仅停止进程内任务，不改变 `auto_start`。
5. 非正常退出无法保证最后一批内存统计全部落库，允许丢失极小窗口的统计增量，但不得损坏规则和日志事实。

## 错误处理

1. 规则不存在、配置无效、运行中编辑、端口绑定失败分别返回明确错误。
2. 启动失败保留规则配置，并在运行状态中保存最后错误。
3. 批量启停返回每条规则的成功或失败结果，不用单个布尔值伪装全部成功。
4. 日志或统计持久化失败不停止仍可正常转发的规则，但必须写入规则级 `last_observability_error` 并在面板提示“转发运行中，日志/统计写入异常”；查询或清理 action 本身不得伪装成功。
5. 若日志/统计失败来自数据库整体不可用，继续转发但不无限缓存待写数据，只保留有界内存错误摘要和实时计数。
6. 运行管理器 mutex 中不执行 bind、connect、join 或数据库 IO，避免长时间持锁和死锁。
7. 不吞掉 Tokio task panic 或 join error；只有在监听和子任务完成清理后才转换为 failed。

## 验证计划

### 纯函数与前端测试

重点覆盖：

1. 协议字段和端点校验。
2. loopback / 局域网监听识别与风险提示。
3. 状态文案和运行中只读判断。
4. HTTP 敏感 Header 脱敏。
5. 文本 Content-Type 判断和 64 KiB 正文截断。
6. 规则列表筛选、端点摘要和批量结果格式化。
7. 组件结构守卫：运行中只读、停止后可编辑、保存与保存并启动分离。

### Rust 单元测试

重点覆盖：

1. 三张表兼容迁移和外键级联。
2. 规则 CRUD、协议字段组合校验和运行中更新拒绝。
3. 日志按规则保留最近 1000 条。
4. 日志清空不重置统计，统计重置不清日志。
5. 启停状态转换、幂等和 `auto_start` 语义。
6. 批量启停的逐条错误隔离。

### 本地 Socket 集成测试

测试只绑定 `127.0.0.1:0` 或测试分配的空闲端口，不依赖公网：

1. HTTP Method、Path、Query、Headers 和 Body 透传。
2. HTTP 逐跳 Header 过滤、转发头生成和敏感日志脱敏。
3. HTTP 大正文流式转发，日志只保留截断预览。
4. HTTPS 下游使用本地测试 TLS 服务和测试证书信任配置。
5. SSE 持续分块转发；WebSocket Upgrade 明确拒绝。
6. 下游连接失败返回 502，超时返回 504。
7. TCP 双向数据、半关闭、下游连接失败和停止断开存量连接。
8. UDP 多客户端响应不串线、空闲会话回收和会话上限。
9. 多条 HTTP/TCP/UDP 规则同时运行。
10. 单条端口冲突不影响其他规则。
11. 自动恢复逐条启动，失败规则显式保留错误。

### 工程验证

实施完成后至少执行：

```powershell
cargo test request_forward -- --nocapture
pnpm test src/utils/requestForward.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

根据实际测试组织补充全量 `cargo test` 和 `pnpm test`。

## 预计接入点

1. `apps/desktop/src/App.vue`：侧边栏入口。
2. `apps/desktop/src/tool-registry.ts`：异步组件注册。
3. `apps/desktop/src/components/RequestForwardPanel.vue`：独立面板。
4. `apps/desktop/src/types/request-forward.ts`：前端类型。
5. `apps/desktop/src/utils/requestForward.ts` 及测试：纯函数。
6. `apps/desktop/src/bridge/tauri.ts`：channel 映射。
7. `apps/desktop/src-tauri/src/tools/request_forward/`：域入口、模型、运行管理器、HTTP/TCP/UDP、日志与测试。
8. `apps/desktop/src-tauri/src/tools/mod.rs`：模块、domain 和 action 契约注册。
9. `apps/desktop/src-tauri/src/tools/helpers.rs`：三张表的兼容迁移。
10. `apps/desktop/src-tauri/src/main.rs` 或现有应用初始化/退出挂钩：共享 runtime、自动恢复和退出清理。
11. `apps/desktop/src-tauri/Cargo.toml`：补充兼容的 HTTP/TLS/取消依赖。

## 风险与取舍

1. 三种协议共享运行管理器，但不共享协议细节，避免形成一个难以测试的大处理函数。
2. Tokio 统一并发和取消模型，代价是需要新增 HTTP/TLS 依赖并明确 runtime 所有权。
3. HTTP 正文日志采用旁路限长观察，不能为了日志破坏流式语义。
4. UDP 必须为客户端维护独立会话，资源开销高于单 socket 转发，但可保证响应不串线。
5. `auto_start` 保留用户期望，实际失败由 runtime 状态表达，避免“数据库写着运行但端口没有监听”的双重真值。
6. 第一版不加入认证、规则改写、负载均衡和 WebSocket，保持范围足以在一个实施计划内完成和验证。

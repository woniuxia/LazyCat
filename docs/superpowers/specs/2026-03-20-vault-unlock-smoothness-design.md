# 密码库解锁顺滑度优化设计

日期：2026-03-20

> 目标：在不调整 PBKDF2 安全参数、不过度重构的前提下，优先改善密码库从输入主密码到主界面可用的“顺滑感”，随后清理已确认的低风险固定开销。首轮只做前两层优化，不实现轻量列表新接口。

## 1. 背景与现状（已在仓库中验证）

### 1.1 前端锁屏存在固定等待与反馈不足

- `apps/desktop/src/components/VaultLockScreen.vue:112-166`
  - 自动解锁当前使用 `attemptedLengths + 500ms debounce`。
  - 自动解锁按“长度”去重，不按“密码值”去重。
- `apps/desktop/src/components/VaultLockScreen.vue:169-224`
  - 自动解锁与手动解锁分属 `isAutoUnlocking` 与 `loading` 两套状态。
  - 自动解锁失败静默，正确密码时用户也看不到“已开始处理”的即时反馈。

### 1.2 已解锁与首屏数据加载当前仍耦合

- `apps/desktop/src/components/VaultPanel.vue:4-15`
  - 锁屏与主界面之间使用 `fade + out-in` 过渡。
- `apps/desktop/src/components/VaultPanel.vue:166-172`
  - 列表项首屏渲染带 `animationDelay: index * 30ms` 的逐项入场延迟。
- `apps/desktop/src/components/VaultPanel.vue:830-867`
  - `checkStatus()` 在发现 unlocked 后会 `await loadEntries()`。
  - `onUnlocked()` 在切 unlocked 后也会 `await loadEntries()`。
  - `loadEntries()` 内部又会 `await loadTagStats()`。
- 结果是：用户即使密码正确，也会感受到“解锁成功后还要再等一段时间才能真正看到界面”。

### 1.3 后端存在可收敛的固定成本

- `apps/desktop/src-tauri/src/tools/helpers.rs:20-35`
  - `get_data_dir()` 每次都会读取并解析 `~/.lazycat/config.json`。
- `apps/desktop/src-tauri/src/tools/helpers.rs:374-377`
  - `db_conn()` 每次连接都执行 `ensure_schema(&conn)`。
- `apps/desktop/src-tauri/src/tools/vault.rs:482-567`
  - `cmd_list()` 逐条调用 `get_entry_tags()`，存在 tags 的 N+1 查询。
- `apps/desktop/src-tauri/src/tools/vault.rs:329-346`
  - `cmd_touch()` 每次续活都会重新开库并读取锁定配置。

### 1.4 安全参数与协议边界本轮保持不动

- `apps/desktop/src-tauri/src/tools/vault.rs:61-67`
  - PBKDF2 迭代次数固定为 `600_000`。
- 本轮不调整 KDF 参数，不改变 `tool:vault:unlock` / `tool:vault:list` / `tool:vault:tag-stats` / `tool:vault:touch` 的前端接口语义。

## 2. 本轮目标与非目标

### 2.1 目标

1. 正确密码输入完成后，用户能更快看到“正在处理”。
2. 解锁成功后，主界面壳层先展示，再异步补列表与标签统计。
3. 降低数据库连接、列表查询和会话续活中的固定重复成本。
4. 保持现有调用链主体不变，优先在既有入口内部收敛状态。

### 2.2 非目标

1. 不调整 PBKDF2 安全参数。
2. 不新增轻量列表接口，不做后续可能的结构性接口拆分。
3. 不改 vault IPC 协议。
4. 不做无关重构，不引入新的数据库访问层。

## 3. 已确认决策

1. 首轮只覆盖“前端体感优化 + 后端低风险性能优化”，不实现新的轻量列表接口。
2. 自动解锁与手动解锁继续复用现有入口：
   - `VaultLockScreen.vue` 中保留 `onUnlock()` / `attemptAutoUnlock()`
   - `VaultPanel.vue` 中保留 `onUnlocked()` / `checkStatus()` / `loadEntries()` / `loadTagStats()`
   - `helpers.rs` 中保留 `db_conn()`
   - `vault.rs` 中保留 `cmd_list()` / `cmd_touch()`
3. 处理中允许继续追加 `unlock` 请求，且允许真并发。
4. 任何一个 unlock 请求成功即可进入已解锁态；旧失败不能覆盖新状态。

## 4. 前端设计

### 4.1 `VaultLockScreen.vue`：收敛解锁请求状态与反馈

#### 4.1.1 保留现有入口，新增统一的内部请求流程

- `VaultLockScreen.vue` 的状态边界保持为：
  - `setup` 模式继续沿用单请求语义；
  - `unlock` 模式引入并发 unlock 的状态归属规则。
- `onUnlock()` 与 `attemptAutoUnlock()` 仍然保留为外部入口。
- 两者都改为调用同一个内部 `runUnlockAttempt(...)` 流程。
- 该流程统一负责：
  - 生成 `attemptId`
  - 记录请求来源（auto / manual）
  - 维护飞行中请求计数
  - 处理成功收敛与错误归属
- `loading` 在实现上可继续复用现有字段，但其并发语义仅作用于 `unlock` 模式；`setup` 流程不引入并发提交，也不复用 unlock 的 attempt bookkeeping。

#### 4.1.2 自动解锁触发改为“更短等待 + 密码值去重”

- 移除当前按“长度”去重的策略。
- 自动解锁改为按“密码值快照”去重：同一密码值不重复自动提交。
- 触发仍保留短 debounce，目标值固定为 **150ms**，只用于等待用户短暂停顿，避免每个字符都触发一次高成本 KDF。
- 自动解锁仍只在 `mode === 'unlock'` 下工作。

#### 4.1.3 允许真并发追加 unlock 请求

根据用户确认，本轮不采用“单飞行请求”限制，而是允许继续并发追加请求：

- 输入框在处理中保持可编辑。
- 自动解锁在新的密码值出现后可以继续发起请求。
- 手动点击/回车也可以在已有请求未返回时继续发起新请求。
- 组件内部维护：
  - `inFlightCount`：当前未完成 unlock 请求数
  - `latestAttemptId`：最近一次发出的请求编号
  - `latestManualAttemptId`：最近一次手动请求编号
  - `hasUnlocked`：是否已有请求成功并完成 UI 收敛

#### 4.1.4 结果归属规则

- 任一请求成功：
  - 若 `hasUnlocked === false`，立即置为 true 并 `emit('unlocked')`。
  - 成功收敛时立即清空 `errorMsg`，并清除尚未触发的 debounce 定时器，避免已成功后又补发新的自动解锁请求。
  - 若组件已卸载，则该成功结果直接丢弃，不再 `emit('unlocked')`，也不再更新任何本地状态。
  - 后续较晚返回的失败或成功一律忽略：不重复 emit，也不再污染 UI。
- 自动解锁失败：
  - 继续静默，不弹错，不清空输入。
- 手动解锁失败：
  - 仅允许“最近一次手动请求”的失败更新 `errorMsg`。
  - 旧请求失败不可覆盖新状态，避免用户已继续输入时旧错误回流。
- `loading` 的新语义：
  - 不再表示“禁止重复触发”，只表示“当前存在至少一个解锁请求正在处理”。

#### 4.1.5 视觉反馈调整

- 正确密码进入处理后，不再额外展示专门的“解锁处理中”反馈。
- 在 `unlock` 模式下，不使用按钮内建 loading 作为并发交互的一部分，避免误导为“不可再次点击”。
- 解锁按钮继续保持可点击，回车也继续允许重复提交，以满足真并发追加请求的交互要求。
- `setup` 模式继续保留单请求语义，不引入可重复并发提交。

### 4.2 `VaultPanel.vue`：将“已解锁”与“数据已加载”解耦

#### 4.2.1 `checkStatus()` / `onUnlocked()` 先切主界面，再补数据

- `checkStatus()` 发现 `unlocked` 时：
  1. 立即 `setLockState('unlocked')`
  2. 立即启动会话计时/续活相关状态
  3. 后台触发 `loadEntries({ phase: 'initial' })`
- `onUnlocked()` 在收到锁屏组件的成功事件时：
  1. 立即切为 unlocked
  2. 不再等待列表完整返回后才显示主界面
  3. 后台触发 `loadEntries({ phase: 'initial' })`
- `startAutoLockCheck()` 中从 locked 恢复到 unlocked 的路径，也统一复用同一契约：先切主界面，再触发 `loadEntries({ phase: 'initial' })`。
- `maybeOpenPendingEntrySeed()` 继续保留，但触发时机延后到首次 `loadEntries({ phase: 'initial' })` 成功之后，避免在主界面刚解锁但首屏列表/标签上下文尚未就绪时过早打开草稿。

这意味着“主界面可见”与“数据加载完成”明确拆分成两个阶段。

#### 4.2.2 引入显式的列表加载态，避免误显示空状态

- 当前 `entries = []` 时会直接落入“还没有凭据”空状态。
- 在 unlocked 后的首次加载阶段，需要单独的 `listLoading` / `entriesLoaded` 语义：
  - **加载中**：显示主界面壳层 + 列表区域轻量加载态
  - **加载完成且为空**：显示真正的“还没有凭据”空状态
- 这样避免用户刚解锁就看到一个误导性的空库文案。
- 首次后台加载期间的 UI 规则明确如下：
  - 左侧导航区整体保持可见，不回退到锁屏；
  - 环境/分类统计先基于当前 `entries` 渲染，首次加载时自然显示为 0，待 entries 返回后再更新；
  - 标签区在 `tag-stats` 返回前继续隐藏，且不显示单独的“暂无标签”空态文案；
  - 右侧列表区显示轻量加载态，而不是直接显示“还没有凭据”。
- 这里明确区分两类加载：
  1. **解锁后的首次后台加载**：允许使用 `listLoading + entriesLoaded=false` 驱动首屏占位，避免误判为空库。
  2. **已在主界面中的普通刷新**（如保存、删除、标签重命名后重新 `loadEntries()`）：
     - 保留旧 `entries`，不回退到首屏加载态；
     - 只做局部刷新，不重新展示“首次加载占位”；
     - 若刷新失败，保留旧数据并沿用现有错误处理，不把页面误切为空态。

#### 4.2.3 `loadEntries()` 与 `loadTagStats()` 解耦

- `loadEntries()` 只负责首屏 entries。
- `loadTagStats()` 后置执行，不阻塞主界面可见。
- 首屏顺序调整为：
  1. 主界面壳层可见
  2. entries 返回后列表可用
  3. tag stats 返回后补全标签侧栏统计
- `tag-stats` 的显示规则：
  - 在其尚未返回时，左栏标签区继续隐藏；
  - 不显示独立的标签加载占位，也不显示“暂无标签”文案；
  - 仅当 `tagStats` 成功返回且长度大于 0 时，才按现有样式显示标签区；
  - 若首屏场景下 `list` 已成功但 `tag-stats` 失败，主列表继续可用，标签区保持隐藏，不升级为阻塞性失败；
  - 普通刷新期间如果已有旧 `tagStats`，则保留旧标签区；若这次 `tag-stats` 刷新失败，则继续保留旧值，不清空主列表。

#### 4.2.4 `loadEntries()` / `loadTagStats()` 的调用契约

- 本轮采用 **`loadEntries()` 主导** 的加载契约：
  - `loadEntries({ phase: 'initial' | 'refresh' })` 负责列表主状态；
  - `loadTagStats({ phase: 'initial' | 'refresh' })` 只负责标签区状态；
  - `loadEntries()` 在列表结果成功提交后按同一 `phase` 以 **fire-and-forget** 方式触发 `void loadTagStats(...)`，不再等待标签统计完成才让主界面进入可用态；调用方不再重复编排两者时序。
- `loadEntries({ phase: 'initial' })` 负责：
  - 置位首屏 `listLoading`；
  - 维持 `entriesLoaded=false` 直到列表结果成功或失败收敛；
  - 成功后写入 `entries`，再触发 `loadTagStats({ phase: 'initial' })`；
  - 若失败但 session 仍有效，结束首屏 loading，切换到轻量错误占位，不误显示为空库。
- `loadEntries({ phase: 'refresh' })` 负责：
  - 保留旧 `entries`；
  - 不回退首屏加载态；
  - 成功后替换列表数据，再触发 `loadTagStats({ phase: 'refresh' })`；
  - 失败时保留旧列表并沿用现有错误处理。
- `loadTagStats({ phase })` 负责：
  - 只更新标签区可见性与标签数据；
  - `initial` 失败时保持标签区隐藏；
  - `refresh` 失败时保留旧标签数据。
- `loadGeneration / requestToken` 仅负责“结果提交保护”，不要求主动取消已发出的请求本身。

#### 4.2.5 `loadGeneration / requestToken` 的提交规则

- 一个 unlocked 周期共享一个 `loadGeneration`。
- 在同一个 unlocked 周期内：
  - `loadEntries()` 维护自己的 `latestListRequestToken`；
  - `loadTagStats()` 维护自己的 `latestTagStatsRequestToken`。
- 每次触发 `loadEntries({ phase: 'initial' | 'refresh' })`，都生成新的 list token；只有“generation 匹配且 token 等于当前最新 list token”的结果才能提交。
- `loadEntries()` 成功后以 fire-and-forget 方式触发 `loadTagStats({ phase })`；后者同样生成新的 tag-stats token，且只有“generation 匹配且 token 等于当前最新 tag-stats token”的结果才能提交。
- 这条规则对应用户确认的口径：**同一 unlocked 周期里只认最后一次刷新结果**。
- 因此在“保存 → 删除 → 重命名”等连续刷新场景中：
  - 如果第 1 次请求最后才返回，其结果必须丢弃；
  - 只有最后一次刷新返回的结果可以覆盖当前页面。

#### 4.2.6 异步结果的代际保护

因为本轮把解锁后的加载改成后台执行，所以需要防止旧请求回写新状态：

- 为 vault 主界面数据加载引入 `loadGeneration` / `requestToken`。
- 每次进入新的 unlocked 周期或重新锁定时递增 generation。
- `loadEntries()` / `loadTagStats()` 只有在：
  - 当前 generation 仍匹配，且
  - 当前 lock state 仍为 unlocked
    时才允许提交结果。
- 该保护同时覆盖两类路径：
  1. **解锁后的首次加载**：防止锁定后旧请求写回。
  2. **已解锁主界面内的后续刷新**：防止较晚返回的旧刷新结果覆盖更新后的新数据。
- 防止出现以下问题：
  - 用户已重新锁定，但旧的 `list/tag-stats` 请求回来后又把数据写回页面；
  - 保存、删除、标签重命名后，新一轮刷新已经开始，但更早的一轮请求晚到并覆盖当前列表。

#### 4.2.7 解锁成功后的动画减负

- 弱化或移除 `Transition` 上的 `mode="out-in"`，避免解锁成功后仍被切场动画额外拉长体感。
- 列表首屏渲染不再使用 `index * 30ms` 的逐项延迟。
- 保留必要的 hover / 细节动效，但不再让首屏可用性依赖逐条动画完成。

### 4.3 会话计时逻辑的顺序调整

- 当前 `startInactivityTimers()` 位于 `loadEntries()` 内。
- 调整后，会话一旦确认 unlocked，就应立即启动计时逻辑，而不是依赖列表成功返回。
- 这里的“立即启动”仅指前端本地的 `startInactivityTimers()`，不额外在 `onUnlocked()` / `checkStatus()` 成功分支中主动补发一次 `touch` 请求。
- 后续 `touchSession()` 仍继续通过现有 `recordVaultActivity()` 链路触发，避免为解锁成功额外增加一次不必要的 IPC。
- 这样即便 `list` 请求较慢，锁定策略仍从真实解锁时刻开始生效。

## 5. 后端设计

### 5.1 `helpers.rs`：`db_conn()` 初始化一次化

#### 5.1.1 区分“连接级初始化”与“进程级 schema 初始化”

- `db_conn()` 继续作为唯一数据库连接入口。
- `db_conn()` 对每个新连接仍执行连接级初始化，至少保留 `PRAGMA foreign_keys = ON` 这类连接级设置，确保外键约束与级联删除行为不回退。
- 仅将 schema / 索引 / FTS / seed 这类进程级初始化，从“每次连接执行”收敛为“进程首次连接执行一次”。
- 并发首连时，使用进程级互斥保护 schema 初始化：
  - 同一时刻只允许一个连接执行进程级初始化；
  - 其他并发连接等待该初始化结果，不能绕过初始化直接返回可用连接。
- 若某次 schema 初始化失败：
  - 本次 `db_conn()` 直接返回错误；
  - **不缓存失败结果**，后续 `db_conn()` 仍继续尝试初始化；
  - 只有当某次初始化完整成功后，才标记进程级初始化完成。
- 不分叉新的数据库层，不改调用方。

#### 5.1.2 `get_data_dir()` 结果做进程级缓存

- `get_data_dir()` 首次解析后缓存结果，后续直接复用。
- 这样避免每次 vault 请求都读取并解析 `config.json`。
- 该缓存对本项目是安全的：
  - `settings.rs:258-265` 与 `settings.rs:268-282` 已明确数据目录切换/重置后要求重启应用。
  - 单个进程生命周期内数据库路径本就不应变化。

### 5.2 `vault.rs`：优化 `cmd_list()` 的 tags 查询

#### 5.2.1 保持返回结构不变

`cmd_list()` 仍返回现有字段：

- `id`
- `category`
- `title`
- `environment`
- `account`
- `summary`
- `tags`
- `createdAt`
- `updatedAt`

前端 `VaultListEntry` 类型不需要变更。

#### 5.2.2 查询策略改为“两段式”而非逐条查 tags

- 第一段：按现有筛选条件查询 `vault_entries`。
- 第二段：收集本批 `entry_id`，一次性查询其 tags。
- 在 Rust 内存中按 `entry_id -> Vec<String>` 组装，再回填到每条结果中。

建议的查询形态：

```sql
SELECT entry_id, tag
FROM vault_entry_tags
WHERE entry_id IN (...)
ORDER BY entry_id, tag
```

#### 5.2.3 职责边界保持不变

- `get_session_key()` 继续负责读取并续活会话密钥。
- `make_summary()` 继续负责从解密字段拼摘要。
- 若需要新增批量 tags helper，仅在 `cmd_list()` 内部或其邻近私有函数使用，不外扩到更大范围。

### 5.3 `vault.rs`：收敛 `cmd_touch()` 的高频 I/O

#### 5.3.1 优先复用 session 中已缓存的 `hard_lock_after_secs`

- `cmd_touch()` 不再在热路径中重新 `db_conn()` 并加载锁定配置。
- 续活请求只负责：
  - 校验 session 是否仍有效
  - 更新 `last_activity`
  - 返回当前 lockState

#### 5.3.2 何时刷新锁定策略

`hard_lock_after_secs` 继续在这些已有路径中刷新：

- `cmd_setup()`
- `cmd_unlock()`
- `cmd_change_password()`

本轮不为 `touch` 增加实时配置刷新机制。

#### 5.3.3 取舍说明

- 优点：显著减少高频 `touch` 的无效数据库访问。
- 取舍：如果用户刚修改了锁定配置，新的硬锁定时长以“下次重新建立/刷新 session”后生效。
- 为保持前后端语义一致，本轮明确采用“**当前已解锁会话冻结既有锁定策略**”的口径：
  - 后端 session 内的 `hard_lock_after_secs` 保持旧值直到下次 `setup/unlock/change_password`；
  - 前端 `VaultPanel.vue` 在当前 unlocked 周期内也继续沿用进入该会话时读取到的锁定策略启动本地计时；
  - 设置页修改锁定预设后，新的前后端策略统一在下一次重新建立/刷新 session 后生效。
- 该取舍可接受，因为本轮目标是低风险减开销，不扩展新的配置同步链路。

## 6. 错误处理与边界条件

### 6.1 解锁并发场景

- 多个 unlock 请求可并发运行。
- 任一成功即可进入 unlocked。
- 旧失败不能覆盖较新的输入状态，更不能在成功后把 UI 改回错误。
- 组件销毁后返回的 unlock 结果不得再更新已卸载组件状态。

### 6.2 主界面异步加载场景

- unlocked 后的 `list` / `tag-stats` 请求为后台请求。
- 若用户在请求返回前重新锁定，旧结果必须被丢弃。
- 若 `list` 失败但 session 已失效，仍通过现有 `handleVaultError()` 收敛到锁屏态。
- 若首屏 `list` 加载失败但 session 仍有效：
  - 结束首屏 `listLoading`；
  - 保留主界面壳层，不回退锁屏；
  - 右侧列表区显示最小错误占位，包含简短失败文案与一个显式“重试”按钮；
  - “重试”按钮直接重新调用 `loadEntries({ phase: 'initial' })`；
  - 不把该失败误呈现为空库。

### 6.3 数据目录缓存边界

- 本轮的 `get_data_dir()` 缓存依赖“切换数据目录后重启”的既有产品约束。
- 不引入运行时热切换数据目录的支持。

## 7. 验证方案

### 7.1 功能验证

1. 正确密码自动解锁：
   - 输入完成后快速出现处理中反馈。
   - 正确密码可正常进入 unlocked。
2. 正确密码手动解锁：
   - 点击/回车可正常解锁。
   - 在允许并发的设计下，旧失败不应覆盖新成功。
3. 错误密码后重试：
   - 自动失败保持静默。
   - 手动失败正常显示错误提示。
4. 会话恢复：
   - `checkStatus()` 发现已解锁时，应先展示主界面壳层，再补列表。
5. 现有交互回归：
   - 标签筛选
   - 复制账号/密码
   - 显示密码
   - 编辑条目
   - 复制为副本
   - 删除条目
   - 标签重命名/删除

### 7.2 体感验证

对比优化前后重点观察：

1. 输入结束到开始解锁反馈的时间。
2. 解锁成功到主界面可见的时间。
3. 解锁成功到列表首屏可用的时间。
4. 空库、少量条目、中大量条目场景下，列表首屏是否仍被逐项动画放大体感。
5. `tag-stats` 明显晚于 `list` 返回时，主界面与列表应已可用，标签区按约定延后补齐且不影响首屏可见。

### 7.3 构建与静态验证

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. Rust / Tauri 侧编译校验（至少覆盖本轮修改的 `helpers.rs` 与 `vault.rs` 所在工程）
4. 如有必要再运行 `pnpm test`
5. 如需最终收口，再补一次完整桌面工程构建验证，确认前后端联编无误

### 7.4 手工路径验证

1. 首次解锁
2. 锁定后再解锁
3. 关闭到托盘后再解锁
4. 密码显示/复制/编辑路径
5. 已解锁主界面内执行删除、保存、标签重命名后，确认页面不会误回到首屏加载态或错误空态
6. 首屏 `list` 失败时应出现最小错误占位；点击“重试”后直接重新调用 `loadEntries({ phase: 'initial' })`

## 8. 实施顺序

1. `apps/desktop/src/components/VaultLockScreen.vue`
   - 自动解锁触发策略
   - 并发 unlock 的状态归属
   - 处理中反馈
2. `apps/desktop/src/components/VaultPanel.vue`
   - unlocked 与数据加载解耦
   - 列表/标签统计后置顺序
   - 首屏加载态
   - 动画减负
3. `apps/desktop/src-tauri/src/tools/helpers.rs`
   - `ensure_schema()` 一次化
   - `get_data_dir()` 缓存
4. `apps/desktop/src-tauri/src/tools/vault.rs`
   - `cmd_list()` 去除 tags N+1
   - `cmd_touch()` 去除热路径重复 I/O

---

确认记录（用户已确认）：

- 首轮严格只做“前端体感优化 + 后端低风险性能优化”，不做新的轻量列表接口
- 保留现有主要入口与调用链，不做大重构
- 不修改 PBKDF2 参数
- 解锁处理中允许继续追加请求，并允许真并发
- 推荐方案采用前端体感优化优先、后端低风险优化其次的顺序

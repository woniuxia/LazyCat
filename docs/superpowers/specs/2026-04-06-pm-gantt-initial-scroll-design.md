# 项目管理甘特图首次进入定位设计

- 日期：2026-04-06
- 范围：`项目管理 > 甘特图视图`
- 状态：评审中，待 spec reviewer 与用户复核

## 1. 背景

当前项目管理甘特图基于 `frappe-gantt` 渲染。用户反馈：每次从看板或其他区域切回甘特图时，视口都会出现一次从左向右的横向滑动动画。

排查后确认：

1. `PmGanttView.vue` 在每次进入甘特图时都会重新创建 `new Gantt(...)` 实例。
2. 当前项目没有显式传入 `scroll_to`，因此继承了 `frappe-gantt` 默认配置 `scroll_to: 'today'`。
3. `frappe-gantt` 内部在处理 `scroll_to` 时，对“滚到今天”使用了 `scrollTo({ behavior: 'smooth' })`。
4. 因此前端每次重新挂载甘特图，都会再次触发一次“平滑滚到今天”的默认行为。

用户已进一步明确期望：

1. 进入甘特图时仍然默认定位到今天附近。
2. 不要看到横向滑动动画。
3. `today` 不应贴在视口最左侧，而应位于当前可视区左侧约三分之一处。

## 2. 改动目标

本次设计的明确目标如下：

1. 去掉每次进入甘特图时的横向平滑滚动动画。
2. 保留“首次进入默认看今天附近”的产品语义。
3. 将 `today` 的默认落点调整为当前视口左侧约三分之一处。
4. 不破坏现有甘特图的日期拖拽、右键、悬浮卡、选中态、日周月切换等既有交互。
5. 不引入后端改动、数据库改动和用户设置项。

## 3. 非目标

本次设计明确不包含以下内容：

1. 不改造为“记住上次离开时的甘特图位置”。
2. 不改为保活甘特图实例或调整 `PmPanel.vue` 的视图挂载策略。
3. 不修改任务条自身的 SVG 宽度动画；本次仅处理整张甘特视口的首次横向滚动。
4. 不 patch `frappe-gantt` 源码，不引入 `patch-package`。
5. 不增加新的设置开关，例如“默认定位到今天”“默认从最左开始”等偏好项。

## 4. 已确认的产品决策

### 4.1 默认落点

进入甘特图时：

1. 若今天位于当前甘特时间范围内，则默认定位到今天附近。
2. `today` 的视觉落点应位于当前可视区左侧约三分之一处，而不是紧贴左边缘。
3. 该定位必须是无动画、直接到位。
4. 验收以“当前日期指示线”的横向位置为准；在未触发边界钳制时，该位置应落在 `viewportWidth * (1 / 3)` 附近，允许误差不超过 `1 * columnWidth`。
5. 若因边界钳制无法满足三分之一落点，则以钳制后结果为准，不视为缺陷。

### 4.2 越界处理

若今天不在当前甘特时间范围内：

1. 不强行滚动到范围之外。
2. 保持甘特图默认起始位置展示。
3. 不额外提示，不新增状态文案。

### 4.3 生效时机

该定位只应发生在“进入甘特图并创建新实例完成后”的首次展示阶段：

1. 从看板切到甘特图时生效。
2. 重新进入项目管理后切到甘特图时生效。
3. 不应在同一实例的普通刷新链路中重复触发。

## 5. 根因与现状

### 5.1 当前根因

当前问题不是项目自定义动画造成，而是第三方库默认行为造成：

1. `frappe-gantt` 默认配置中，`scroll_to` 的默认值为 `today`。
2. 当 `scroll_to` 为 `today` 时，库内部会进一步调用“滚到当前日期”的逻辑。
3. 最终滚动通过 `scrollTo({ behavior: 'smooth' })` 完成，因此会出现明显横向滑动。

### 5.2 当前项目触发条件

`PmGanttView.vue` 当前在进入甘特图时：

1. 重新 `innerHTML = ""`
2. 重新创建 `new Gantt(...)`
3. 未覆盖 `scroll_to`

因此每次重新进入甘特图，都会完整走一次第三方库默认的“平滑滚到今天”流程。

### 5.3 依赖验证结论

本设计基于当前仓库锁定的 `frappe-gantt@1.2.2` 本地源码结论：

1. 当前安装版本为 `1.2.2`，包内包含 `src/` 源码。
2. 在该版本的 `set_scroll_position(date)` 中：
   - 当 `!date || date === 'start'` 且 `infinite_padding` 为真时，会直接执行 `this.$container.scrollLeft = min_start` 并 `return`
   - 不会进入后续 `scrollTo({ behavior: 'smooth' })` 分支
3. `frappe-gantt@1.2.2` 默认 `infinite_padding` 为 `true`，但本次不依赖默认值，而是在 PM 甘特图创建时显式传入 `infinite_padding: true`
4. 因此，将 `scroll_to` 显式设为 `'start'` 且同时显式传入 `infinite_padding: true`，可以确定性地关闭默认“滚到 today 的平滑滚动”，并把基础落点固定在库认可的左侧起点。
5. 在该版本的 `get_closest_date()` 中，范围判断语义是：
   - 当 `now < gantt_start || now > gantt_end` 时返回 `null`
   - 因此 `gantt_start` 与 `gantt_end` 均按包含端点处理
6. 在该版本的渲染流程中，会调用 `highlight_current()`；当今天在甘特范围内时，会生成 `.current-highlight` 当前日期指示线并追加到 `.gantt-container`
7. 若今天不在甘特范围内，则不会生成 `.current-highlight`
8. 因此，本次自定义初始定位以 `.current-highlight` 的真实 DOM 坐标作为 today 锚点，不再镜像 `date_utils.diff()` 算法。

## 6. 方案对比

### 6.1 方案 A：项目层接管首次定位

做法：

1. 创建甘特图时显式关闭默认 `scroll_to: 'today'`
2. 在 `PmGanttView.vue` 中读取 today 指示线的真实位置作为目标锚点
3. 直接赋值 `scrollLeft`，无动画到位

优点：

1. 改动局部，仅影响 PM 甘特图
2. 不引入第三方补丁维护成本
3. 可以精确满足“左侧三分之一处”这个产品目标

缺点：

1. 需要依赖 `frappe-gantt` 当前生成的 `.current-highlight` DOM 结构
2. 需要额外补一层计算和测试

结论：**采用此方案**

### 6.2 方案 B：修改第三方库，增加无动画滚动参数

做法：

1. 修改 `frappe-gantt` 的 `set_scroll_position()`
2. 将 `behavior: 'smooth'` 改为可配置
3. 项目层传入“无动画滚动到今天”的配置

不采用原因：

1. 需要维护第三方 patch
2. 升级依赖时要重新核对补丁
3. 当前问题可以在项目层安全解决，没有必要抬高维护成本

### 6.3 方案 C：保活甘特图实例，避免每次重建

做法：

1. 调整 `PmPanel.vue` 视图切换方式
2. 避免甘特图在切换时被卸载重建

不采用原因：

1. 这是更大范围的生命周期调整
2. 它更偏“保留上次位置”，不是“默认到今天附近”
3. 改动面超出本次问题边界

## 7. 设计方案

### 7.1 组件职责

本次只调整前端局部职责：

`PmGanttView.vue` 负责：

1. 创建甘特图实例时覆盖默认 `scroll_to`
2. 在首次渲染完成后执行无动画初始定位
3. 继续保留现有右键、悬浮卡、选中态、拖拽、视图切换逻辑

`pmGantt.ts` 负责：

1. 提供“首次进入时的目标 `scrollLeft`”纯函数
2. 封装基于 `currentX` 的三分之一视口偏移与边界钳制规则

### 7.2 首次定位策略

创建 `new Gantt(...)` 时，不再使用库默认 `scroll_to: 'today'`。

改为：

1. 显式传入 `scroll_to: 'start'`
2. 同时显式传入 `infinite_padding: true`
3. 甘特图 DOM 就绪后，由项目层执行一次无动画定位
4. 定位方式为直接设置内部 viewport 的 `scrollLeft`
5. 为每个新建实例维护一次性保护标记，例如 `didApplyInitialScroll`
6. 该标记在以下两类路径都必须置为已处理：
   - 已成功设置目标 `scrollLeft`
   - 已判定无需处理或已明确放弃处理
7. 标记置位后，不再允许同实例中的刷新、resize 或视图切换再次触发首次定位

这样可以彻底避开第三方库内部的 `behavior: 'smooth'`。

### 7.3 目标位置计算

“今天所在的横向位置”以 `frappe-gantt` 渲染后生成的真实当前日期指示线为准，而不是在 PM 侧镜像一套日期差值算法。

目标计算规则如下：

1. `todayX` 的定义固定为：`.current-highlight` 在 `.gantt-container` 坐标系中的真实横向位置
2. 当 `.current-highlight` 存在时，读取其实际 `left` / `offsetLeft` 作为 `currentX`
3. 当 `.current-highlight` 在补偿重试后仍不存在时，视为“今天不在当前甘特范围内”，不执行额外滚动
4. `viewportWidth` 明确定义为横向滚动容器 `.gantt-container` 的 `clientWidth`
5. 目标滚动值使用：
   - `targetScrollLeft = currentX - viewportWidth / 3`
6. 最终再对结果做边界钳制：
   - 最小值为 `0`
   - 最大值为 `scrollWidth - viewportWidth`
7. 当 `scrollWidth <= viewportWidth` 时，视为当前图表无需横向滚动，最终结果等价于 `0`

这里的“三分之一”是产品目标位置，而不是额外再叠加库内部原来的 `column_width / 6` 偏移。

### 7.4 尺寸未就绪时的处理

为了减少“先出现在最左边，再跳到目标位置”的闪动感，定位时机按以下优先级执行：

1. 优先在实例创建完成后的同一轮流程中立即尝试定位
2. 若首次尝试满足以下任一条件，则判定为“尺寸未就绪”，补一次 `requestAnimationFrame`
   - 未找到内部 viewport
   - `viewportWidth <= 0`
   - `scrollWidth <= 0`
3. 若首次尝试发现 `scrollWidth <= viewportWidth`，则判定为“无需横向滚动的稳定态”，直接以 `0` 作为结果并置位 `didApplyInitialScroll`
4. 若首次尝试仅缺少 `.current-highlight`，则补一次 `requestAnimationFrame`，用于区分“DOM 尚未稳定”与“today 真实越界”
5. 第二次尝试若 viewport 仍未就绪，则放弃本次初始定位，保留 `scroll_to: 'start'` 已落下的起始位置，并置位 `didApplyInitialScroll`
6. 第二次尝试若 `.current-highlight` 仍不存在，则按“today 越界”处理，不执行额外滚动，并置位 `didApplyInitialScroll`
7. 放弃时不弹 toast，不新增界面提示，不触发额外重试

该策略的目标是：

1. 尽量在首帧前完成定位
2. 只在必要时延后一帧
3. 不引入多次重试和额外状态复杂度

### 7.5 生效边界

首次定位逻辑只在“新实例渲染完成后”触发，不进入以下链路：

1. 已有实例的 `refresh()`
2. `change_view_mode(mode, true)` 视图粒度切换
3. 拖拽日期后的乐观刷新
4. 选中态切换
5. popup 重定位

也就是说，现有“保持当前滚动位置”的逻辑继续保留，本次不覆盖。

### 7.6 第三方依赖边界

本次允许读取、但不允许覆写的 `frappe-gantt` 内部依赖边界固定为：

1. DOM：
   - 继续复用现有 `getGanttViewport()`，读取内部 `.gantt-container`
   - `.gantt-container` 本身就是用户看到的横向滚动容器，也是最终被写入 `scrollLeft` 的 DOM 节点
   - 允许读取 `.current-highlight` 当前日期指示线的真实位置

约束如下：

1. 不新增对更多实例私有字段或未公开模块的依赖
2. 若 viewport 或 `.current-highlight` 的读取条件不满足，则按既定降级路径处理
3. 不为此增加 UI 级错误提示
4. 后续若升级 `frappe-gantt` 造成 `.current-highlight` 结构变化，应以手工回归与集成验证发现并修正

## 8. 数据流与接口边界

### 8.1 新增纯函数

必须在 `apps/desktop/src/utils/pmGantt.ts` 中新增单一职责纯函数 `computePmGanttInitialScrollLeft`：

1. 输入：
   - `currentX`
   - `viewportWidth`
   - `scrollWidth`
2. 输出：
   - 已钳制好的目标 `scrollLeft`

该纯函数只负责三分之一偏移与边界钳制，不感知 DOM，不感知 Vue 状态，也不负责判断 today 是否越界。

### 8.2 组件侧调用

`PmGanttView.vue` 侧只负责：

1. 通过现有 `getGanttViewport()` 读取内部 `.gantt-container`
2. 读取 `.current-highlight` 的真实横向位置
3. 调用纯函数获取目标值
4. 执行 `viewport.scrollLeft = targetScrollLeft`
5. 若 viewport 不可用，则直接跳过本次初始定位，保持当前起始位置，并置位 `didApplyInitialScroll`
6. 若 `.current-highlight` 在补偿后仍不存在，则按 today 越界处理，不滚动并置位 `didApplyInitialScroll`

这样可以保持：

1. 公式逻辑集中
2. DOM 操作集中
3. 单测与 UI 行为测试边界清晰

## 9. 测试设计

### 9.1 单元测试

必须在现有 `apps/desktop/src/utils/pmGantt.test.ts` 中补充纯函数测试，至少覆盖以下场景：

1. 当 `currentX` 位于中间区域且未触发边界钳制时，返回值应满足“当前日期指示线”落在 `viewportWidth * (1 / 3)` 附近，允许误差不超过 `1px`
2. 当目标值小于 `0` 时，结果被钳到 `0`
3. 当目标值超过最大滚动值时，结果被钳到最大值
4. 当 `scrollWidth <= viewportWidth` 时返回 `0`

### 9.2 手工回归

实现后必须至少完成以下回归：

1. 从看板切到甘特图，多次重复进入，确认不再出现横向平滑滚动
2. `Day / Week / Month` 三种视图下都确认 `today` 落点大体位于左侧三分之一处
3. 当今天超出甘特范围时，确认展示默认起始位置
4. 拖拽任务日期后，确认不会再次触发首次进入定位
5. 切换任务选中、右键菜单、悬浮卡时，确认现有交互不受影响
6. 确认首次进入时未调用任何 `scrollTo({ behavior: 'smooth' })` 路径，且最终定位依赖的是直接写入 `scrollLeft`
7. 确认 `today` 越界时，`.current-highlight` 不存在且组件不会重复尝试初始定位

### 9.3 最低验证命令

实现完成后，最低必须执行：

1. `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
2. `pnpm typecheck`

## 10. 风险与缓解

### 10.1 风险：依赖第三方 DOM 结构细节

本次方案需要依赖 `frappe-gantt` 当前生成的 DOM 结构，例如：

1. `.gantt-container`
2. `.current-highlight`

缓解方式：

1. 只读取既有 DOM，不覆写第三方内部方法
2. 将依赖点收敛到 `PmGanttView.vue` 的一次性定位流程中
3. 用纯函数单测守住滚动钳制逻辑，用手工回归守住 DOM 结构集成点

### 10.2 风险：首帧尺寸未稳定导致定位失败

缓解方式：

1. 先同步尝试
2. 再补一次 `requestAnimationFrame`
3. 若仍失败则保守退回默认起点，不做反复重试

### 10.3 风险：误伤现有滚动保持链路

缓解方式：

1. 首次定位逻辑只放在新实例创建完成后
2. 不进入 `refresh()` 和 `change_view_mode(..., true)` 链路
3. 手工回归覆盖切视图、拖拽改期和已有滚动恢复

## 11. 实施摘要

本次设计的最终实施方向可以压缩为一句话：

进入项目管理甘特图时，项目层显式关闭 `frappe-gantt` 默认的 `scroll_to: 'today'` 平滑滚动，并在实例渲染完成后，基于库自身时间轴语义直接计算并设置 `scrollLeft`，让 `today` 以无动画方式落在当前可视区左侧约三分之一处；已有刷新、拖拽和视图切换的滚动保持逻辑不变。

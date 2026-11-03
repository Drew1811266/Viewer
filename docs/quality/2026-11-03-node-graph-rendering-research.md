# 计划画布的图渲染与布局选型

> Status: Research draft（已决策）
>
> 日期：2026-11-03
>
> 目的：为 [生成计划设计](../superpowers/specs/2026-11-02-viewer-generation-plan-graph-design.md)
> 的第一层画布选定渲染与布局方案。
>
> **决定：引入 `@xyflow/react` + `@dagrejs/dagre`。** 两者均为 MIT，已在许可证白名单内。

## 1. 需求

| 维度 | 要求 |
| --- | --- |
| 交互 | 只读为主：平移、缩放、选中、下钻；后续可能演化出编辑 |
| 形状 | 当前是树：计划 → 组 → 分镜；未来可能出现多父/交叉 |
| 规模 | 画布常驻 3–10 节点；展开一组 ≤ 60；上限 50 组 / 500 分镜 |
| 位置 | **确定性**：同一份计划永远同一套坐标，跨版本不漂移 |
| 外观 | 完全自定义（M3 卡片、状态描边、角标、声明数量警告） |
| 可访问性 | 键盘可达、焦点顺序合理、读屏可读节点与状态 |
| 依赖 | 通过 `scripts/check-npm-licenses.mjs` 白名单；不新增网络请求 |

白名单：MIT、MIT-0、Apache-2.0、Apache-2.0 OR MIT、MIT OR Apache-2.0、BSD-2-Clause、
BSD-3-Clause、ISC、CC0-1.0、MPL-2.0、BlueOak-1.0.0。

## 2. 候选与结论

| 项目 | 版本 | 许可证 | 角色 | 结论 |
| --- | --- | --- | --- | --- |
| [`@xyflow/react`](https://github.com/xyflow/xyflow) | 12.12.1 | MIT | 画布框架 | **采用** |
| [`@dagrejs/dagre`](https://github.com/dagrejs/dagre) | 3.1.3 | MIT | 分层布局 | **采用** |
| [`elkjs`](https://github.com/kieler/elkjs) | 0.13.0 | EPL-2.0 OR GPL-3.0-or-later | 分层布局 | **不可用**：不在白名单，需走依赖例外 |
| [`d3-hierarchy`](https://github.com/d3/d3-hierarchy) | — | ISC | 树布局 | 备选：若未来退回纯树且想减依赖 |
| [Cytoscape.js](https://github.com/cytoscape/cytoscape.js) | — | MIT | 图分析与渲染 | 不采用：能力远超需要 |

`@xyflow/react@12.12.1` 的传递依赖：`@xyflow/system@0.0.84`（精确版本）、`classcat@^5`、
`zustand@^4`，均为 MIT。peer 要求 React ≥ 17。

## 3. 采用后的分工

| 由库负责 | 由我们负责 |
| --- | --- |
| 视口：平移、缩放、自动适配 | 布局参数：层距、节点间距、列对齐 |
| 节点容器、选中、框选、键盘焦点管理 | 节点组件（M3 卡片、状态描边、角标） |
| 边的绘制与锚点计算 | 边的语义（belongs / uses / references）与样式 |
| 变更时的增量更新 | 由计划推导 `nodes[]` / `edges[]` 的纯函数 |
| 可见区域裁剪（`onlyRenderVisibleElements`） | 与标记、状态、抽屉的联动 |

**布局仍由我们配置**：dagre 决定坐标，我们决定层方向（自上而下）、间距与排序规则。
同一份计划 + 同一套参数 → 同一套坐标，跨版本不漂移。

## 4. 集成清单

### 4.1 治理（缺一不可）

1. `ui/package.json` 增加 `@xyflow/react` 与 `@dagrejs/dagre`。
2. `THIRD_PARTY_NOTICES.md` 为两者各加一行（策略测试按依赖名断言）。
3. `scripts/repository-policy.test.mjs` 的 `directDependencies` 冻结清单加两项——
   这是**有意的治理变更**，提交时需要在 ADR 或提交说明里写清楚。
4. `pnpm-lock.yaml` 更新；`pnpm security` 重跑，确认许可证清单通过。

两者均为 MIT，白名单内，**不需要**新增 `dependency-exceptions.json` 条目。

### 4.2 技术

1. **样式隔离**：引入 `@xyflow/react/dist/style.css`，其类名与 CSS 变量均以 `react-flow__` /
   `--xy-` 为前缀；在计划工作台根节点下作用域化，避免与现有 token 冲突。
2. **确定性**：`nodesDraggable={false}`、`nodesConnectable={false}`、`elementsSelectable`、
   位置全部来自 dagre；节点使用**固定宽高**，避免首帧 DOM 测量导致位置抖动。
3. **节点组件**：`nodeTypes` 注册三种：`plan` / `group` / `shot`，内部复用现有 M3 primitives。
4. **可访问性补偿**：React Flow 的键盘与读屏支持有限。节点容器补 `role`、`aria-label`、
   roving tabindex；边不进入 Tab 序列；在抽屉里提供等价的结构化列表。
5. **测试环境**：jsdom 缺 `ResizeObserver`、`DOMMatrix`，需在 `ui/src/setupTests.ts` 补 polyfill，
   否则画布组件无法在 Vitest 中挂载。
6. **渲染规模**：启用 `onlyRenderVisibleElements`，大计划下只渲染可见节点。

## 5. 风险与应对

| 风险 | 应对 |
| --- | --- |
| `zustand@4` 与 React 19 的兼容性未验证 | 先做一个 spike：挂载 50 个节点、验证选中与缩放；不通过则锁定 React Flow 版本或评估替代 |
| 首帧节点测量抖动，破坏"坐标由结构推导" | 所有节点给固定宽高；位置只来自 dagre，不启用自动布局插件 |
| 归因链接 | React Flow 默认在画布角落显示品牌链接。MIT 不强制移除，但官方请求保留或订阅 Pro。**产品决策：保留小字归因**，如需移除单独评审 |
| 库的升级破坏自定义节点契约 | 版本跟随 `^12`，升级前跑画布组件的视觉与交互测试 |

## 6. 参考

- xyflow 许可证：https://github.com/xyflow/xyflow/blob/main/LICENSE
- `@xyflow/react` 包信息：https://registry.npmjs.org/@xyflow/react/latest
- dagre：https://registry.npmjs.org/@dagrejs/dagre/latest
- elkjs 许可证（被排除）：https://registry.npmjs.org/elkjs/latest
- 仓储许可证白名单：`scripts/check-npm-licenses.mjs`

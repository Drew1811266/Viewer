# 持续评审 Task 18：大图工作台与动态素材范围

> Status: Development evidence
>
> 2026-08-30：Task 18 已完成实现、独立复核和完整门禁。实现提交 `c6d5d11`。
> 阶段 A–D 加 Task 17–18，累计完成 18 / 23；下一步是 Task 19。
> 阶段 E 尚未整体完成，本记录不表示持续评审 UI 已在生产入口启用。

## 范围与架构

仅在 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`、
`codex/continuous-review-domain` 实施，基线 `ddb58f7`。未修改主工作区，未合并、推送、迁移
真实工程或修改原始媒体；未进入 Task 19，也未接通真实存档／历史面板。

工作台通过 `imageReviewWorkbenchAdapter.ts` 消费 Task 17 协调器，不直接依赖 Viewer bridge。
legacy 与 continuous 显式分派：当前生产调用仍默认 legacy，continuous 入口只在组件、Hook 和
真实 coordinator 测试载体中启用。几何层仍只理解 Anchor 和本地 Item ID，不理解协议状态。

身份边界如下：

| 概念 | 用途 | 不可替代 |
| --- | --- | --- |
| Feedback ID | 自然语言意见身份 | 不作为画布编号或 Target ID |
| Target ID | 工作台本地 Item ID、定位与选择 | 不作为 Feedback ID |
| Target Revision Key | 编辑、重绘、删除的完整并发依据 | 不由显示编号或当前列表重建 |
| Ordinal | 当前素材上的显示编号 | 不持久化为协议身份 |
| preparation/session key + Asset Version ID | 草稿与精确预览所有权 | 不由 entityId 或相同路径推断 |

## 行为与完整性契约

1. v3 新打开图片动态准备对应 Asset Version，可直接新增自然语言意见；图 1 保存后打开图 2，
   画笔、矩形和整图意见仍可用。网格浏览不创建反馈，不存在 completed-round 锁。
2. continuous 使用准备结果中的精确预览 URL；preview 的 Asset Version 与准备资产不一致时
   禁止编辑。普通邻图预取在 continuous 工作台关闭，输入、切工具或状态变化不会重载当前图。
3. 新建草稿和已有意见都冻结 entity、session/preparation key 与 Asset Version；已有意见额外
   冻结 Feedback ID 和完整 Target Revision Key。同名实体、同路径覆写、会话替换或并发修订
   均不能把旧草稿写入新上下文。
4. 文字编辑只更新对应 Feedback；重绘和删除只操作选中的 Target Revision。保存前再次与当前
   snapshot 精确比较，版本变化要求重新选择，不能用最新列表中的同名 Target 覆盖并发更新。
5. 明确未提交的失败保留文字和标记；原载荷重试复用底层输入，用户修改后可明确提交新载荷。
   取消草稿同时清理 coordinator editor；不确定结果／恢复状态不能被取消动作绕过。
6. 状态文案按 entity + Asset Version + 当前 projection 计算：`已保存，待确认` 会在目标可用后
   更新为 `已保存，可供外部读取`，不跨图、跨版本泄漏，也不声称 Agent 已读或已执行。
7. continuous 工具栏只有“存档意见”“历史”；回调留给 Task 19/20。legacy 的完成／放弃流程和
   原有文案保持不变，未新增保存／删除状态提示。删除最后一条不显示“全部通过”。
8. UI 内实际承载 Target 的字段统一命名为 `*ItemId`；协议层 legacy `feedbackId` 仍只表示真正的
   Feedback ID，避免维护时混淆身份。

## RED → GREEN 与独立复核

RED→GREEN 日志保留在 `target/continuous-review-task18-*.log`。回归覆盖路由、动态准备、精确
预览、保存／失败／待确认、文字编辑、重绘、删除、显式 mutation context、跨图／跨会话草稿、
来源变化、失败后取消或改写、稳定预览、状态归属、紧凑布局、键盘、缩放／旋转／导航和 legacy。

独立只读复核未发现 Critical。首轮发现并复现的 Important 均已修复：

- preparation 未绑定 session 实例，以及 adapter 重建后 mutation 依赖闭包上下文；
- 编辑开始后未冻结完整 Target Revision，可能覆盖并发文字／几何修订；
- 保存失败后本地取消未清理底层 editor，后续编辑会被软锁；
- prepared preview 对象不稳定且 continuous 触发无效邻图预取；
- 同 entity 会话替换时新意见草稿可能写入新会话；
- 操作成功文案可能跨素材／版本泄漏；
- `beginDrawing` 闭包遗漏 feedback 依赖，文字保存后立即重绘可能携带旧 revision。

修正后的独立复验确认无 Critical、Important 或可执行 Minor。最后两项维护性问题也已关闭：
状态随 projection 重算，Target 承载字段改为 `*ItemId`。独立复验未修改文件、索引或 HEAD。

## 验证证据

- Task 18 聚焦回归：18 个文件 / 157 项通过。
- UI 全仓：140 个文件 / 1235 项通过，1 个既有跳过。
- `pnpm --dir ui check` 通过；仅有仓库既有 Biome recommended 配置弃用提示。
- `pnpm architecture:boundaries`、contracts、桌面安全边界通过。
- `pnpm architecture:trends` exit 0 / 48 项非阻断告警，未修改基线；本任务新增工作台 Hook 仍
  超过趋势建议阈值，职责已通过 adapter、annotation model 和纯 helper 分层，Task 19 不得继续
  将存档／历史状态塞入该 Hook，应由独立面板／协调器职责承接。
- 最终 `pnpm verify:clean` exit 0；Rust workspace、格式、Clippy、协议、构建、安全、许可和
  仓库洁净门禁通过。

完整门禁首跑时，一个本轮未修改的 `VideoPreview` 全屏退出测试在全仓并发负载下超时一次；
同一 UI 全仓在此前已全绿。按系统化调试单独复现该测试及完整文件：目标用例单跑通过，完整
22 项连续运行 5 次、共 110 项全部通过，相关路径相对基线无 diff。未对无关视频代码做猜测性
修改；第二次完整 `verify:clean` 的 UI 140 文件 / 1235 项和其余全部门禁均通过。

## 后续边界

下一步仅是 Task 19：接入精确手动存档预览、部分范围和后补保护 UI。Task 20 才接历史／恢复／
来源确认，Task 21 才在全部阶段 E 门禁完成后切换真实入口。当前 continuous 工作台仍不能用于
真实旧工程迁移，Task 18 的 archive/history 只保留注入点。

代码签名、Apple 公证、正式安装包、上架、公开发布和发售不属于当前任务、阻塞或验收条件。

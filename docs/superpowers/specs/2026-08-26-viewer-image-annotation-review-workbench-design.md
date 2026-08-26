# Viewer 图片标注评审工作台设计

> Status: Active
>
> 日期：2026-08-26
>
> 类型：AI 素材评审路线阶段 3 架构与交互设计
>
> 当前事实边界：本文描述下一轮待实施能力，不表示 Viewer 当前已经支持图片画笔、区域
> 标注、标注预览图或 `viewer.review/2`。当前行为仍以
> [`PRODUCT_SPEC.md`](../../PRODUCT_SPEC.md) 为准。

## 1. 背景与问题

阶段 2 已实现例外驱动人工评审闭环：用户固定本轮图片／视频范围，只为需要返工的素材
记录自然语言意见，明确完成后发布不可变 Completed Round，外部 Agent 可以精确读取结果。

当前交互仍以“开始评审 → 选择素材 → 打开独立意见面板 → 添加意见”为中心。真实用户更
常见的判断过程是先在批量网格浏览，在发现一张可能有问题的图片后打开大图，再针对具体
位置逐条说明。独立意见面板切断了“看到问题、指出位置、描述修改”的连续动作；当一张图
存在多处问题时，纯整图文字也要求用户额外解释每句话对应的位置。

阶段 3 把图片大图预览升级为精确评审的主要工作台，同时保留阶段 2 已验证的固定名单、
Draft 恢复、冲突阻断、明确完成、默认通过和 Agent 无关读取语义。

## 2. 设计目标

- 用户从批量网格进入大图后，可以直接用画笔或矩形选区指出问题位置，并在标记附近输入
  自然语言意见。
- 第一次保存有效意见时，自动使用进入大图前的当前浏览范围建立评审 Draft，不要求用户
  先理解或操作独立的“开始评审”流程。
- 同一图片可以同时包含多个局部意见和整图意见；每条意见仍以用户自然语言原文为权威。
- 保存后的标记以编号和可收起意见栏呈现，避免多条永久文字气泡遮挡素材。
- 外部 Agent 同时获得结构化 Anchor 和带编号标记的只读预览图；原始素材保持不变。
- 保持图片加载／视口、评审会话、标注几何、协议序列化和资源渲染之间的窄边界。
- 历史 `viewer.review/1` Completed Round 继续可读，新记录通过明确版本升级表达新能力。

## 3. 非目标

本轮不包含：

- 视频时间点或时间段标注；只为未来视频 Anchor 保留扩展边界；
- 多人实时协作、评论回复、审批角色或云同步；
- 标注颜色、线宽、字体或其他装饰设置；
- AI 自动识别问题、改写意见、生成摘要或修改原始图片；
- OCR、箭头、椭圆、套索、模糊或像素编辑工具；
- 同时维护多个可写人工评审 Draft；
- 代码签名、Apple 公证、正式安装包、公开发布或发售。Viewer 当前仍处于开发初期，
  这些事项不是本轮的计划项、验收项、阻塞项或收尾事项。

## 4. 已确认的产品决策

1. 第一次保存有效图片意见时，Viewer 自动冻结进入大图前的当前浏览范围并创建 Draft；
   不显示开始评审确认对话框。
2. 局部意见和整图意见并存。一个区域标记对应一条独立自然语言 Feedback。
3. 采用“编号标记 + 就地输入 + 可收起右侧意见栏”的界面方案。
4. 结构化 Anchor 是权威定位数据；Completed Round 同时提供带编号标记的只读预览图。
5. 本轮完整实现图片区域标注；视频时间标注后续独立设计和实施。
6. 浏览、画笔和矩形选区是互斥工具模式，空格只临时切换到平移。
7. 只有非空文字与有效 Anchor 一起保存成功后才形成 Feedback；空标记不持久化，也不产生
   `revise`。
8. “返回网格”只关闭大图；“完成本轮评审”是独立入口，并始终经过汇总确认。
9. 当前 Draft 的成员名单保持固定。范围外图片不能被静默加入，也不能在项目内创建第二个
   并行人工 Draft。
10. 矩形可以移动和缩放；画笔路径通过整体重绘替换，不提供逐节点编辑。
11. 浏览行为不参与 Outcome 计算。图片在批量网格中可见就足以让用户完成快速判断，不要求
    逐张进入大图；显式完成时，无 Feedback 且可评审的固定素材仍按默认通过处理。

## 5. 用户工作流

### 5.1 没有活动 Draft

```text
批量浏览当前范围
  → 打开一张图片
  → 选择画笔／矩形／整图意见
  → 创建临时标记并输入文字
  → 保存
  → 后端固定进入大图时捕获的浏览范围
  → 原子创建含首条 Feedback 的 Draft
  → 显示“已创建本轮评审，共 N 项”轻量提示
```

临时标记和文字在首次保存成功前只存在于 React 编辑状态。准备被取消、项目只读、写入租约
忙、范围变化或保存失败时，不创建空 Draft；用户输入保留并可以重试。

`ReviewScopeSnapshot` 在进入大图时捕获有序 Entity ID、Folder Projection 语义、后代开关和
项目 generation。之后的滚动、选择或筛选变化不能改写该快照；Application 仍在首次保存时
根据这些身份重新解析素材事实，前端不能提交路径或自行构造 Asset Version。

### 5.2 已有活动 Draft

- 本轮图片打开后加载全部已保存 Anchor，并按稳定顺序显示编号。
- 用户可以继续添加整图、矩形或画笔意见，或者编辑文字、调整矩形、重绘画笔、删除意见。
- 上一张／下一张、返回网格和重开项目都从已保存 Draft 恢复相同标记。
- 批量网格只为存在 Feedback 的图片显示“返工 · N 条”；无意见图片不显示“通过”控件。
- 多选素材的共同整图意见继续使用阶段 2 的批量入口；精确区域意见只在单图工作台创建。

### 5.3 范围外图片

如果用户在活动 Draft 期间打开不属于固定名单的图片，普通预览继续可用，但标注工具不可写，
界面明确提供“返回本轮素材”“完成当前评审”和“放弃当前草稿后重新开始”。Viewer 不自动
扩展本轮，也不猜测用户是否希望建立另一轮。

### 5.4 离开与完成

未保存的标记或文字会阻止切换图片、返回网格、关闭项目和完成本轮。用户只能继续编辑或
明确放弃该临时内容。

“返回网格”只关闭大图。“完成本轮评审”在网格和大图中都可见，两个入口调用同一完成用例。
汇总确认显示固定素材、返工素材、Feedback、不可评审和默认通过数量，并列出全部意见；
确认前不发布任何 `pass`。完成用例不要求逐张打开大图，也不读取曝光、停留或滚动遥测。

## 6. UI 架构与组件职责

### 6.1 组合边界

`ImagePreview` 保持通用图片渲染器职责：加载表示、缩放、平移、旋转、放大镜、导航和源图／
舞台坐标转换。它不导入 Review API，不持有 Draft，不序列化 Anchor，也不决定 Outcome。

新增 `ImageReviewWorkspace` 作为组合层：

- `ImagePreview`：现有图片与视口能力；
- `AnnotationCanvas`：临时／已保存几何、命中检测、选择和视口投影；
- `AnnotationToolbar`：浏览、画笔、矩形三种互斥工具及“整图意见”命令；
- `InlineFeedbackEditor`：新意见的就地临时输入；
- `ReviewFeedbackRail`：编号列表、定位、文字编辑、重绘和删除；
- `ImageReviewSessionAdapter`：把 UI 意图转换为评审协调器命令，不直接调用 Tauri。

`ReviewSessionCoordinator` 继续管理 Round、revision、任务进度、恢复、冲突、完成和只读状态。
它接收已经归一化并通过前端基本约束的 Anchor DTO，但不处理指针采样或画布布局。

### 6.2 布局

- 正常宽度窗口中，意见栏停靠在大图右侧并触发画布重排，不覆盖图片；收起后画布恢复全宽。
- 紧凑窗口中，意见栏变为可关闭覆盖抽屉，默认只显示编号标记。
- 新标记完成后，文字编辑器出现在标记附近；保存后编辑器消失，画面仅保留编号。
- “整图意见”不进入第四种指针模式，而是在意见栏打开无 Anchor 几何的输入区；保存后只在
  列表中显示，不在图片上伪造区域标记。
- 点击编号会选择意见栏条目；点击条目会高亮并定位对应标记。
- 标记使用固定高对比样式；Domain 不保存颜色或线宽偏好。

编号由单张图片中 Feedback 的确定顺序派生，不是持久身份；Feedback ID 才是编辑、删除和
协议引用的稳定身份。同一快照内画布、意见栏、JSON 编号映射和预览图必须使用同一次编号
投影，删除后允许剩余编号在新快照中连续重排。

### 6.3 工具与键盘

- `V`：浏览模式；
- `B`：画笔模式；
- `R`：矩形选区模式；
- 空格：标注期间临时平移；
- `Esc`：取消当前临时操作并返回浏览模式；
- `Command + Enter`：保存当前意见。

输入框获得焦点时不处理工具快捷键。工具按钮、编号和意见列表都提供明确名称、焦点顺序和
状态。矩形支持键盘微调位置和尺寸；自由画笔属于指针输入，整图意见始终提供完整键盘替代。

## 7. 几何模型

### 7.1 Anchor 类型

`viewer.review/2` 是 v1 Anchor 的严格超集，Feedback Target 支持：

```text
asset
imageRect   { x, y, width, height }
imageStroke { points[] }
videoPoint  { positionUs }
videoRange  { startUs, endUs }
```

`imageRect` 和 `imageStroke` 只能指向具有已确认非零宽高的图片 Asset Version。本轮工作台
只新建 `asset`、`imageRect` 和 `imageStroke`；`videoPoint`／`videoRange` 为兼容既有数据而
继续可读写，但本轮不增加它们的 UI。

### 7.2 坐标空间

所有图片坐标都归一化到校正显示方向后的源图 `[0, 1] × [0, 1]`。保存值不依赖窗口尺寸、
设备像素比、适应窗口比例、自由缩放、平移或临时旋转。

UI 通过 `ImageViewportGeometry` 窄接口完成：

```text
stage point ↔ oriented source point ↔ normalized source point
```

标注时即使图片被旋转，提交前也必须反向映射到规范源图坐标。重新渲染时再把规范坐标投影到
当前视口。任何无法建立正反映射的状态都禁用标注，不得保存近似位置。

### 7.3 画笔路径

指针移动只在前端采样。抬笔后按确定性算法去除重复点并进行有界简化，再验证：

- 至少两个不同点；
- 每个点有限且位于闭区间 `[0, 1]`；
- 点数不超过协议上限；
- 路径包围盒具有非零面积；
- 序列化大小不超过请求和协议上限。

简化后的路径是权威 Anchor。线宽只用于 Viewer 显示和预览渲染，不参与返工语义。

## 8. Domain 与 Application

### 8.1 Domain

Review Domain 扩展显式的图片 Anchor 值对象，并继续维护：

- Feedback 文字非空、时间有效、目标非空且不重复；
- Target 必须属于同一 Round 的固定 Asset Version；
- 图片 Anchor 只能指向图片，并满足媒体边界；
- Draft 中 Feedback 的添加、替换、删除保持确定顺序；
- Completed Outcome 仍按 `revise → unreviewable → pass` 优先级计算；
- Completed Round 不可原地修改。

Domain 不读取图片、不计算视口坐标、不渲染 PNG、不知道 React、Tauri 或具体 Agent。

### 8.2 首条意见用例

新增原子应用用例 `start_review_with_feedback`，接收：

- 精确项目会话和 generation；
- 打开大图时捕获的 `ReviewScopeSnapshot`；
- 当前图片 Entity ID；
- 非空自然语言；
- 一个合法 Anchor；
- 请求 ID 和取消令牌。

用例在一个 Repository 写入租约内重新检查没有活动人工 Draft，准备并固定完整范围，构造含
首条 Feedback 的 Draft，然后只保存一次。不能先保存空 Draft 再补意见。

### 8.3 后续变更

添加、编辑、重绘和删除采用统一的 clone-validate-save-swap：

```text
校验 roundId + expected revision
  → clone 当前 Draft
  → 在副本执行 Domain 行为
  → Repository 原子保存副本
  → 保存成功后替换内存状态并递增 revision
```

画笔重绘原子替换原 Anchor 并保留 Feedback ID、原文字和创建时间。矩形移动／缩放也只通过
同一替换行为完成。保存失败时已持久化 Draft 和内存快照不变，React 临时编辑状态可以重试。

删除最近意见后，React 提供有界、会话级撤销；撤销本身仍是一次带 expected revision 的正式
Application 变更。关闭项目后不保留撤销栈。

## 9. 协议版本与兼容

### 9.1 v1

现有 `viewer.review/1` Completed 文件保持逐字节不可变。v1 Draft 可以恢复；发生第一次 v2
编辑时在同一写入租约内解码为 Domain、升级并原子写回 v2，保持 Project、Stream、Round、
Asset Version 和 Feedback 身份。v1 `imageRegion` 确定性映射为等坐标的 v2 `imageRect`，
`asset`、`videoPoint` 和 `videoRange` 原样保留；任一 Anchor 无法无损映射时整次升级失败，旧
Draft 保持不变。

### 9.2 v2 Catalog、Draft 与 Round

新增严格 JSON Schema：

- `viewer-review-index-v2.schema.json`；
- `viewer-review-draft-v2.schema.json`；
- `viewer-review-round-v2.schema.json`。

Catalog v2 为每个 Completed Round 记录 Round ID、协议版本、受限相对位置和摘要；第一次发布
v2 Round 时从 v1 Catalog 原子迁移：读取并验证每个既有 v1 Round 后，为其补入明确位置和
摘要，保留全部历史顺序和每个 Stream 的精确 head。任何历史文件缺失或无效都会阻止迁移。
旧 v1 读取器必须稳定拒绝 v2 Catalog，更新后的参考读取器同时验证 v1 和 v2，不能猜测文件
位置或回退到项目级“最新”。

### 9.3 v2 Round Bundle

逻辑布局为：

```text
.viewer/reviews/
├─ index.json
├─ drafts/
│  └─ <review-round-id>.json
└─ rounds/
   ├─ <legacy-v1-round-id>.json
   └─ <v2-round-id>/
      ├─ round.json
      └─ artifacts/
         └─ <asset-version-id>-annotation.png
```

v2 Completed JSON 为每个生成的预览资源保存 Asset Version ID、受限相对路径、摘要、宽和高。
只有存在局部 Anchor 的图片需要预览资源；整图文字仍由结构化 Feedback 完整表达。

## 10. 标注预览生成与发布事务

标注预览使用 Viewer 已授权的源图片表示和校正方向，最长边与总像素有明确上限。它包含源图
可见内容、区域轮廓、画笔路径和编号，不把完整意见文字烧入像素；JSON 中的编号映射和文字
保持权威且可搜索。

完成流程为：

```text
锁定评审变更
  → 重验全部固定素材
  → 在受保护临时目录渲染所需 PNG
  → 校验尺寸、MIME、摘要和完整引用
  → 生成 Completed round.json
  → fsync bundle 文件与临时目录
  → 原子 rename 整个 Round Bundle
  → fsync rounds 父目录
  → 原子更新 Catalog v2
  → fsync Catalog 与 reviews 父目录
  → 核对 exact Stream head
  → 删除 Draft
```

Round Bundle 已发布但 Catalog 尚未更新时，它是可验证孤儿。下次打开使用现有线性孤儿恢复
原则，只在 Project、Stream、previous head、Round ID、资源引用和摘要全部唯一吻合时恢复。
否则进入 `RecoveryRequired`。临时目录、缺失资源、摘要不匹配或多条分叉永远不能成为 head。

## 11. 错误、冲突与安全边界

- 只读项目、另一个 Viewer 持有写入租约或评审数据版本不受支持时，普通浏览和图片预览继续
  可用；标注保存明确失败且保留本地未保存内容。
- 当前图片不属于固定本轮时，标注入口只读，不自动扩展范围。
- 素材被替换、移动、删除，内容摘要变化，或已确认图片尺寸／方向变化时产生 Version
  Conflict 并阻止完成。旧 Anchor 不迁移到新内容。
- 前端只能提交 Entity／Asset Version 身份和有界 DTO，不能提交项目路径、缓存路径或任意输出
  位置。
- Repository 拒绝 `.viewer/reviews/` 所有权边界内的符号链接、非规范大小写、未知文件、路径
  穿越、重复身份和超限文件。
- 预览 PNG 只使用本地授权素材和固定渲染规则，不加载网络资源、不执行用户文字，也不写回
  原素材。
- 外部 Agent 只读取 Catalog 收录并完全验证的 Completed Bundle；Draft、临时资源和孤儿都
  不是返工指令。

## 12. 性能边界

- 指针移动和实时路径显示完全在前端完成，不逐点 IPC。
- 抬笔后在前端进行有界简化；保存只发送一次有界 Anchor 请求。
- 已保存标记从 Draft 快照投影，不为每一帧重新序列化或访问磁盘。
- 意见栏开合触发一次视口重排，Anchor 通过同一几何接口重投影。
- 首条意见的素材准备继续使用阶段 2 的有界并发、进度和取消机制。
- 标注预览只在完成准备阶段生成，Draft 的普通编辑不反复输出 PNG。
- Feedback 数量、单路径点数、单 Round 总点数、文字长度、资源尺寸和 Bundle 总大小均设硬
  上限，并在前后端共同验证。

## 13. 验收与测试

### 13.1 Domain 与协议

- `asset`、`imageRect`、`imageStroke` 的合法／非法边界、媒体类型和重复目标；
- 归一化点、矩形、路径简化和协议上限的属性测试；
- 具有图片 Anchor 的 Feedback 必须产生 `revise`；默认通过语义不变；
- v1 Completed 逐字节不改写，v1 Draft 单向升级，v1/v2 Catalog 和 Round 严格验证；
- 缺失、替换、超限、符号链接或摘要不匹配的预览资源被拒绝。

### 13.2 Application 与 Repository

- 首条意见只能产生“没有 Draft”或“含完整首条 Feedback 的 Draft”两种结果；
- 准备取消、租约竞争、保存失败和进程中断没有空 Draft 或部分名单；
- 添加、编辑、矩形变换、画笔重绘、删除和撤销的 revision 竞争；
- Round Bundle 发布前后每个故障点的恢复矩阵；
- Catalog 迁移保留所有 Stream 和 v1 历史，且不产生项目级最新；
- 范围外素材、内容替换和图片边界变化阻止写入或完成。

### 13.3 React 与视觉验收

- 浏览、画笔、矩形和临时平移的指针／键盘所有权；
- 只在批量网格看过素材也能完成，未进入大图不改变默认通过语义；
- 50%–400% 缩放、平移、临时旋转和意见栏重排下的坐标往返；
- 一张图片连续添加至少 4 条局部／整图意见；
- 就地编辑器、保存失败、未保存离开确认、编号／列表双向定位；
- 矩形移动缩放、画笔整体重绘、删除与撤销；
- 网格“返工 · N 条”、大图“返回网格”和两个位置的统一“完成本轮评审”；
- 正常、紧凑、200% UI 缩放、键盘焦点和屏幕阅读器名称的验收状态。

### 13.4 端到端验收

使用真实高分辨率图片完成：

```text
从 30 张批量网格进入第 6 张
  → 画出 2 条路径、2 个矩形并分别保存文字
  → 返回网格并看到“返工 · 4 条”
  → 重开项目恢复 Draft
  → 重新打开第 6 张并准确恢复 4 个 Anchor
  → 完成本轮
  → 在 30 张均可评审时得到 29 张 pass、1 张 revise、0 张 unreviewable
  → Agent 读取器验证 v2 Round、4 条意见和标注预览摘要
```

最终必须从待实施提交重新运行完整 `pnpm verify:clean`。环境依赖的原生媒体测试继续按仓库
既有条件报告，不得把未运行项目写成通过。

## 14. 文档影响

实施完成前，本文保持 Active 设计状态，当前产品文档不得把图片区域标注写成已实现。

实施完成后至少同步：

- `docs/PRODUCT_SPEC.md`；
- `docs/protocol/README.md` 和 v2 JSON Schema；
- `docs/product/USER_GUIDE.md`；
- `docs/product/FEATURE_REFERENCE.md`；
- `docs/product/SHORTCUTS.md`；
- `docs/product/DATA_PRIVACY.md`；
- `docs/product/TROUBLESHOOTING.md`；
- `docs/README.md` 与 `CHANGELOG.md`。

完成报告不得把签名、公证、正式安装包、公开发布或发售列为默认后续事项。

# Viewer 产品文档维护规范

> Status: Current
>
> 当前开发版本：`0.1.6`

本规范用于功能新增、调整、重构或版本推进后同步产品文档。原则是先审计运行代码和测试，再更新
当前产品描述；历史规划继续保留为历史证据，不能反向覆盖新版本事实。

## 1. 当前文档集合

| 文档 | 主要职责 |
| --- | --- |
| `docs/PRODUCT_SPEC.md` | 产品定位、用户、边界、信息架构、主流程和当前功能总览 |
| `docs/product/USER_GUIDE.md` | 面向使用者的端到端操作步骤和恢复入口 |
| `docs/product/FEATURE_REFERENCE.md` | 每个功能域的状态、行为、边界和维护证据 |
| `docs/product/SHORTCUTS.md` | 当前代码实际接管的键盘、鼠标和触控板输入 |
| `docs/product/SUPPORTED_FORMATS.md` | 扫描候选、预览/解码能力、限制和失败分类 |
| `docs/product/DATA_PRIVACY.md` | 源数据、`.viewer`、缓存、设置、网络和安全边界 |
| `docs/product/TROUBLESHOOTING.md` | 现象、原因、安全恢复和副作用范围 |
| `CHANGELOG.md` | 按版本归纳已经发生的用户可见或维护边界变化 |
| `README.md` | 仓库级产品简介、版本、启动和主要文档入口 |
| `docs/README.md` | 所有当前、工程和历史文档的状态索引 |

### 当前开发阶段与计划边界

- Viewer 目前处于开发初期，当前交付范围是源码开发、内部开发验证，以及与变更直接相关
  的代码、测试和文档。
- 代码签名、Apple 公证、正式安装包或 DMG、应用商店上架、公开发布和发售不属于当前
  开发范围，也不是功能开发、缺陷修复或架构优化的默认完成条件。
- 设计、实施计划、验收清单、评审结论和完成报告不得把上述事项自动列为待办、阻塞、
  风险、遗留项或建议的下一步。
- 只有项目负责人明确宣布进入发布阶段，或对某一具体事项单独授权时，才可以规划和执行
  对应的发布工程工作。单项授权不等于项目阶段整体改变。
- 历史文档中已有的签名、构建或发布记录只作为历史证据，不自动转化为当前开发义务。

## 2. 每个版本的更新顺序

1. **确定基线。** 明确要描述的 tag 或完整提交，记录版本号和事实日期；不要从工作区记忆推断。
2. **先审代码。** 核对 UI、状态、Tauri 命令、Rust 服务、格式分类、配置和实际测试，再阅读旧文档。
3. **更新产品规格与功能参考。** 先改 `PRODUCT_SPEC.md` 和 `FEATURE_REFERENCE.md`，统一能力状态和边界。
4. **更新可见操作。** 只有入口、步骤、标签、快捷键或反馈发生变化时，才改 `USER_GUIDE.md` 和
   `SHORTCUTS.md`；每个按键都要有当前事件所有者和测试依据。
5. **更新边界文档。** 格式、编码、大小、存储、网络、权限、缓存或错误恢复变化时，同步
   `SUPPORTED_FORMATS.md`、`DATA_PRIVACY.md` 和 `TROUBLESHOOTING.md`。
6. **补版本记录。** 从 Git 区间和已验证功能整理 `CHANGELOG.md`，不把计划中未落地内容列入完成项。
7. **同步入口和状态。** 更新根 `README.md`、`docs/product/README.md`、`docs/README.md` 的版本、
   链接和 Current/Active/Historical/Superseded 状态。
8. **执行文档与仓库门禁。** 运行产品文档策略、历史范围策略、链接/版本扫描和完整仓库验证。
9. **归档过程文档。** 完成后的设计、计划和评审保留原内容并标记 Historical；不要删除以伪装历史，
   也不要继续把它们列作当前产品事实源。

## 3. 变更影响矩阵

| 代码或配置区域 | 必须复查的产品文档 | 重点问题 |
| --- | --- | --- |
| `ui/src/components/` | `USER_GUIDE`、`FEATURE_REFERENCE`、`SHORTCUTS`、`TROUBLESHOOTING` | 入口、标签、布局、反馈、焦点和可恢复状态是否变化 |
| `ui/src/state/`、`ui/src/app/` | `FEATURE_REFERENCE`、`SHORTCUTS`、`USER_GUIDE` | 输入所有权、会话、选择、搜索、撤销和预览状态机是否变化 |
| `ui/src/api/types.ts` | 全部聚焦文档，优先 `FEATURE_REFERENCE`、`SUPPORTED_FORMATS` | 用户可见类型、状态、错误或限制是否新增/删除 |
| `src-tauri/src/commands/`、`src-tauri/src/state/` | `FEATURE_REFERENCE`、`DATA_PRIVACY`、`TROUBLESHOOTING` | 命令能力、权限、路径、错误映射、缓存和生命周期是否变化 |
| `crates/viewer-infrastructure/src/scan/file_classifier.rs` | `SUPPORTED_FORMATS`、`PRODUCT_SPEC` | 扩展名集合和候选分类是否变化；不要误写成解码保证 |
| 文本读取/Markdown 渲染 | `SUPPORTED_FORMATS`、`USER_GUIDE`、`DATA_PRIVACY`、`TROUBLESHOOTING` | 编码、10 MiB 边界、HTML/资源/链接策略是否变化 |
| `crates/viewer-infrastructure/src/portable/` | `DATA_PRIVACY`、`FEATURE_REFERENCE`、`TROUBLESHOOTING` | `.viewer` 文件、模式、迁移、备份和只读行为是否变化 |
| 文件操作、日志和恢复 | `USER_GUIDE`、`FEATURE_REFERENCE`、`DATA_PRIVACY`、`TROUBLESHOOTING` | 冲突、部分成功、取消、恢复和可撤销种类是否变化 |
| `scripts/video/runtime.lock.json`、视频运行时/原生适配 | `SUPPORTED_FORMATS`、`FEATURE_REFERENCE`、`TROUBLESHOOTING`、`DATA_PRIVACY` | 版本、网络开关、失败类型、播放/表面行为和缓存是否变化 |
| `crates/viewer-application/src/settings.rs`、`SettingsDialog` | `USER_GUIDE`、`FEATURE_REFERENCE`、`DATA_PRIVACY` | 默认值、可选项、持久化模式和清理行为是否变化 |
| `src-tauri/tauri.conf.json`、`capabilities/`、`SECURITY.md` | `DATA_PRIVACY`、`PRODUCT_SPEC`、根 `README` | 平台、CSP、前端权限、网络和交付边界是否变化 |

表中的文档名均指“当前文档集合”中的完整路径。一个跨域改动可能需要复查多行，不能只更新最先
看到的用户指南段落。

## 4. 事实与证据规则

- 当前行为以版本基线的运行代码、类型和用户界面为第一事实源。
- 自动化测试证明被覆盖的合同；原生性能、VoiceOver、真实大型项目等仍需区分“测试存在”和
  “已在目标机器人工验收”。
- 配置和运行时锁定文件证明平台、权限、依赖版本和构建选项。
- 设计稿、原型、历史需求、实施计划和评审用于解释过程，不单独证明当前产品能力。
- 所有数量、单位、扩展名、编码、按键和菜单文字必须能指向当前代码或测试。
- “支持”必须说明是扫描识别、预览、实际解码、编辑还是文件整理，不能把这些层次混为一谈。

## 5. 文档状态

| 状态 | 含义 | 使用规则 |
| --- | --- | --- |
| Current | 与当前产品版本同步的用户/产品文档 | 必须声明当前开发版本并通过产品文档策略 |
| Active | 仍在执行或约束当前工程工作的设计、计划、ADR、研究 | 不等于产品功能已经实现 |
| Historical | 已完成、已迁移或只保留过程证据 | 不作为当前行为主来源 |
| Superseded | 已由明确的新文档替代 | 必须在索引中给出替代文档 |

版本完成后，相关设计与实施计划通常从 Active 调整为 Historical；架构决策只有被新 ADR 明确取代
时才标记 Superseded。

## 6. 提交前检查

至少执行：

```bash
node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs
node scripts/check-scope-coverage.mjs
rg -n '当前开发版本：`0\.1\.6`' docs/PRODUCT_SPEC.md docs/product/*.md
rg -n '\]\([^)]*\.md(?:#[^)]*)?\)' README.md docs/PRODUCT_SPEC.md docs/product docs/README.md
git diff --check
pnpm verify
```

推进版本时，把示例中的 `0.1.6` 改成新版本。完整验证失败时先修复第一个真实根因；不要通过删除
现有测试、降低策略或把 Current 文档改成模糊措辞来绕过不一致。

## 7. 评审清单

- [ ] 所有 Current 文档声明同一版本。
- [ ] 根 README、Tauri 配置、Cargo/UI 包版本和 changelog 一致。
- [ ] 用户指南中的每个标签和快捷键仍可在当前 UI/测试找到。
- [ ] 扩展名、编码、大小、缓存预算和运行时版本与代码一致。
- [ ] 数据文档没有把可重建缓存描述成必须备份，也没有淡化 `.viewer` 的价值。
- [ ] 故障排除明确每个清理/重试动作的影响范围。
- [ ] `docs/README.md` 收录每个规格文档且状态唯一。
- [ ] 新设计/计划已在完成后归档为 Historical，旧文档仍可追溯。
- [ ] 本轮计划和收尾没有把签名、公证、正式安装包、发布或发售误列为默认任务。
- [ ] `git diff --check` 与 `pnpm verify` 均从当前提交重新运行并通过。

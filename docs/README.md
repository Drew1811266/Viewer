# Viewer Documentation Index

本索引按文档权威类型区分当前产品事实、当前工程约束和历史过程证据。文档状态只说明
治理用途，不等于产品正式发布或全部目标完成。

## 当前产品文档

| Document | Status | Replaced by |
| --- | --- | --- |
| [PRODUCT_SPEC.md](PRODUCT_SPEC.md) | Current | — |
| [product/README.md](product/README.md) | Current | — |
| [product/USER_GUIDE.md](product/USER_GUIDE.md) | Current | — |
| [product/FEATURE_REFERENCE.md](product/FEATURE_REFERENCE.md) | Current | — |
| [product/SHORTCUTS.md](product/SHORTCUTS.md) | Current | — |
| [product/SUPPORTED_FORMATS.md](product/SUPPORTED_FORMATS.md) | Current | — |
| [product/DATA_PRIVACY.md](product/DATA_PRIVACY.md) | Current | — |
| [product/TROUBLESHOOTING.md](product/TROUBLESHOOTING.md) | Current | — |
| [product/DOCUMENTATION_MAINTENANCE.md](product/DOCUMENTATION_MAINTENANCE.md) | Current | — |
| [CHANGELOG.md](../CHANGELOG.md) | Current | — |

## 当前工程与治理文档

| Document | Status | Replaced by |
| --- | --- | --- |
| [TECHNICAL_FOUNDATIONS.md](TECHNICAL_FOUNDATIONS.md) | Active | — |
| [OPEN_SOURCE_RESEARCH.md](OPEN_SOURCE_RESEARCH.md) | Active | — |
| [protocol/README.md](protocol/README.md) | Active | — |
| [quality/DEPENDENCY_HEALTH.md](quality/DEPENDENCY_HEALTH.md) | Active | — |
| [architecture/viewer-0.1-api-baseline.md](architecture/viewer-0.1-api-baseline.md) | Active | — |
| [2026-08-24-viewer-architecture-governance-and-workspace-orchestration-design.md](superpowers/specs/2026-08-24-viewer-architecture-governance-and-workspace-orchestration-design.md) | Active | — |
| [2026-08-25-viewer-ai-material-review-workflow-design.md](superpowers/specs/2026-08-25-viewer-ai-material-review-workflow-design.md) | Active | — |
| [2026-08-25-viewer-exception-driven-review-loop-design.md](superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md) | Active | — |
| [2026-08-26-viewer-image-annotation-review-workbench-design.md](superpowers/specs/2026-08-26-viewer-image-annotation-review-workbench-design.md) | Active | — |
| [2026-08-27-viewer-continuous-review-and-agent-handoff-design.md](superpowers/specs/2026-08-27-viewer-continuous-review-and-agent-handoff-design.md) | Active | — |
| [2026-08-27-viewer-continuous-review-and-agent-handoff.md](superpowers/plans/2026-08-27-viewer-continuous-review-and-agent-handoff.md) | Active | — |
| [2026-08-26-viewer-image-annotation-review-workbench.md](superpowers/plans/2026-08-26-viewer-image-annotation-review-workbench.md) | Active | — |
| [2026-08-25-viewer-exception-driven-review-loop.md](superpowers/plans/2026-08-25-viewer-exception-driven-review-loop.md) | Active | — |
| [2026-08-25-viewer-open-review-protocol-foundation.md](superpowers/plans/2026-08-25-viewer-open-review-protocol-foundation.md) | Active | — |
| [2026-08-24-viewer-architecture-boundary-governance.md](superpowers/plans/2026-08-24-viewer-architecture-boundary-governance.md) | Active | — |
| [2026-08-24-viewer-workspace-orchestration-refactor.md](superpowers/plans/2026-08-24-viewer-workspace-orchestration-refactor.md) | Active | — |
| [video-runtime-build.md](video-runtime-build.md) | Active | — |
| [adr/0001-macos-image-pipeline.md](adr/0001-macos-image-pipeline.md) | Active | — |
| [adr/0002-file-transaction-protocol.md](adr/0002-file-transaction-protocol.md) | Active | — |
| [adr/0003-scan-search-and-generation.md](adr/0003-scan-search-and-generation.md) | Active | — |
| [adr/0005-continuous-development-governance.md](adr/0005-continuous-development-governance.md) | Active | — |
| [adr/0006-continuous-review-snapshot-and-archive-protocol.md](adr/0006-continuous-review-snapshot-and-archive-protocol.md) | Active | — |

AI 素材评审路线阶段 2 已于 2026-08-26 完成：设计文档内部状态为 `Implemented`，实施计划
内部状态为 `Complete`。两份文档继续以 Active 治理资料保留，用于约束未来阶段的依赖边界。

持续评审与 Agent 交接完整设计已于 2026-08-27 获用户确认；实施计划已拆为六个阶段、23 个
任务；阶段 A 的纯 Domain 任务 1–5 已在授权的独立分支完成并通过复审，证据见
[阶段记录](reviews/2026-08-27-continuous-review-domain-phase-a.md)。阶段 B、C 现也已完成：
任务 1–15 已通过各自全仓门禁与独立复验，累计 15 / 23。2026-08-28 已完成获批的
Node 入口 + Rust 只读核心，见[阶段 D 读取器记录](reviews/2026-08-28-continuous-review-reader-phase-d.md)；
Task 16 的契约补充已获确认，桥接实现进入全仓验证与独立复核，见[Task 16 检查点](progress/2026-08-28-continuous-review-task16-contract-checkpoint.md)及[桥接验证记录](reviews/2026-08-28-continuous-review-bridge-phase-d.md)。
阶段 E/F 尚未开始，阶段 D 尚未整体完成。架构调整及原失败事实见
[阶段 D 架构检查点](progress/2026-08-27-continuous-review-phase-d-architecture-checkpoint.md)。此前已通过结果见
[阶段 C 记录](reviews/2026-08-27-continuous-review-application-phase-c.md)，阶段 B 结果见
[阶段 B 记录](reviews/2026-08-27-continuous-review-persistence-phase-b.md)，此前暂停状态见
[暂停检查点](progress/2026-08-27-continuous-review-phase-b-paused-checkpoint.md)。任务 6
新增 [v3 状态 Schema](protocol/viewer-review-state-v3.schema.json)、
[索引 Schema](protocol/viewer-review-index-v3.schema.json)、
[存档 Schema](protocol/viewer-review-archive-v3.schema.json)、
[读取结果 Schema](protocol/viewer-review-read-result-v3.schema.json)与
[使用声明 Schema](protocol/viewer-review-usage-v1.schema.json)，均为 Active 工程契约。
Active 不表示产品已解除完成锁定或切换到新协议；迁移与对外读取器仅在隔离工程验证，UI 和桌面组装尚未接入。

## 历史目标、设计与实施资料

这些资料用于解释需求、设计和实施过程，不单独证明 0.1.6 当前产品行为。

### 历史目标与已替代决策

| Document | Status | Replaced by |
| --- | --- | --- |
| [milestones/viewer-0.1-target-requirements.md](milestones/viewer-0.1-target-requirements.md) | Historical | [PRODUCT_SPEC.md](PRODUCT_SPEC.md) |
| [milestones/viewer-0.1-scope-matrix.md](milestones/viewer-0.1-scope-matrix.md) | Historical | [PRODUCT_SPEC.md](PRODUCT_SPEC.md) |
| [adr/0004-viewer-0.1-architecture-freeze.md](adr/0004-viewer-0.1-architecture-freeze.md) | Superseded | [adr/0005-continuous-development-governance.md](adr/0005-continuous-development-governance.md) |

### 设计规格

| Document | Status | Replaced by |
| --- | --- | --- |
| [2026-07-16-viewer-foundation-hardening-design.md](superpowers/specs/2026-07-16-viewer-foundation-hardening-design.md) | Historical | — |
| [2026-07-16-viewer-m3-organization-comparison-design.md](superpowers/specs/2026-07-16-viewer-m3-organization-comparison-design.md) | Historical | — |
| [2026-07-16-viewer-system-architecture-design.md](superpowers/specs/2026-07-16-viewer-system-architecture-design.md) | Historical | — |
| [2026-07-20-viewer-internal-pointer-drag-design.md](superpowers/specs/2026-07-20-viewer-internal-pointer-drag-design.md) | Historical | — |
| [2026-07-20-viewer-marquee-selection-design.md](superpowers/specs/2026-07-20-viewer-marquee-selection-design.md) | Historical | — |
| [2026-07-22-viewer-radial-menu-contextmenu-fallback-design.md](superpowers/specs/2026-07-22-viewer-radial-menu-contextmenu-fallback-design.md) | Historical | — |
| [2026-07-22-viewer-simplified-workspace-radial-menu-design.md](superpowers/specs/2026-07-22-viewer-simplified-workspace-radial-menu-design.md) | Historical | — |
| [2026-07-23-viewer-engineering-optimization-governance-design.md](superpowers/specs/2026-07-23-viewer-engineering-optimization-governance-design.md) | Historical | — |
| [2026-07-23-viewer-folder-filmstrip-rows-design.md](superpowers/specs/2026-07-23-viewer-folder-filmstrip-rows-design.md) | Historical | — |
| [2026-07-27-viewer-aspect-aware-thumbnail-density-design.md](superpowers/specs/2026-07-27-viewer-aspect-aware-thumbnail-density-design.md) | Historical | — |
| [2026-07-27-viewer-light-preview-design.md](superpowers/specs/2026-07-27-viewer-light-preview-design.md) | Historical | — |
| [2026-07-27-viewer-readme-design.md](superpowers/specs/2026-07-27-viewer-readme-design.md) | Historical | — |
| [2026-07-27-viewer-toolbar-menu-buttons-design.md](superpowers/specs/2026-07-27-viewer-toolbar-menu-buttons-design.md) | Historical | — |
| [2026-07-27-viewer-unified-preview-theme-design.md](superpowers/specs/2026-07-27-viewer-unified-preview-theme-design.md) | Historical | — |
| [2026-07-28-viewer-adaptive-text-panel-design.md](superpowers/specs/2026-07-28-viewer-adaptive-text-panel-design.md) | Historical | — |
| [2026-07-28-viewer-development-launcher-design.md](superpowers/specs/2026-07-28-viewer-development-launcher-design.md) | Historical | — |
| [2026-07-28-viewer-intelligent-compare-layout-design.md](superpowers/specs/2026-07-28-viewer-intelligent-compare-layout-design.md) | Historical | — |
| [2026-07-29-viewer-other-files-and-split-text-preview-design.md](superpowers/specs/2026-07-29-viewer-other-files-and-split-text-preview-design.md) | Historical | — |
| [2026-07-30-viewer-complete-ui-visual-upgrade-design.md](superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md) | Historical | — |
| [2026-07-30-viewer-visual-fidelity-correction-design.md](superpowers/specs/2026-07-30-viewer-visual-fidelity-correction-design.md) | Historical | — |
| [2026-07-31-viewer-visual-atlas-final-remediation-design.md](superpowers/specs/2026-07-31-viewer-visual-atlas-final-remediation-design.md) | Historical | — |
| [2026-08-02-viewer-atlas-to-product-complete-migration-design.md](superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md) | Historical | — |
| [2026-08-02-viewer-single-instance-development-launcher-design.md](superpowers/specs/2026-08-02-viewer-single-instance-development-launcher-design.md) | Historical | — |
| [2026-08-03-viewer-native-acceptance-controller-design.md](superpowers/specs/2026-08-03-viewer-native-acceptance-controller-design.md) | Historical | — |
| [2026-08-04-viewer-tiered-visual-acceptance-design.md](superpowers/specs/2026-08-04-viewer-tiered-visual-acceptance-design.md) | Historical | — |
| [2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress-design.md](superpowers/specs/2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress-design.md) | Historical | — |
| [2026-08-05-viewer-progressive-folder-loading-design.md](superpowers/specs/2026-08-05-viewer-progressive-folder-loading-design.md) | Historical | — |
| [2026-08-06-viewer-compare-original-images-and-eight-item-limit-design.md](superpowers/specs/2026-08-06-viewer-compare-original-images-and-eight-item-limit-design.md) | Historical | — |
| [2026-08-06-viewer-five-level-thumbnail-slider-design.md](superpowers/specs/2026-08-06-viewer-five-level-thumbnail-slider-design.md) | Historical | — |
| [2026-08-06-viewer-square-image-card-selection-design.md](superpowers/specs/2026-08-06-viewer-square-image-card-selection-design.md) | Historical | — |
| [2026-08-06-viewer-trackpad-zoom-and-image-magnifier-design.md](superpowers/specs/2026-08-06-viewer-trackpad-zoom-and-image-magnifier-design.md) | Superseded | [2026-08-07-viewer-pointer-following-magnifier-revision-design.md](superpowers/specs/2026-08-07-viewer-pointer-following-magnifier-revision-design.md) |
| [2026-08-07-viewer-fit-relative-original-preview-revision-design.md](superpowers/specs/2026-08-07-viewer-fit-relative-original-preview-revision-design.md) | Historical | — |
| [2026-08-07-viewer-pointer-following-magnifier-revision-design.md](superpowers/specs/2026-08-07-viewer-pointer-following-magnifier-revision-design.md) | Historical | — |
| [2026-08-07-viewer-stable-progressive-preview-and-larger-magnifier-design.md](superpowers/specs/2026-08-07-viewer-stable-progressive-preview-and-larger-magnifier-design.md) | Historical | — |
| [2026-08-09-preview-original-single-reveal-design.md](superpowers/specs/2026-08-09-preview-original-single-reveal-design.md) | Historical | — |
| [2026-08-09-viewer-bottom-shelf-preview-loading-design.md](superpowers/specs/2026-08-09-viewer-bottom-shelf-preview-loading-design.md) | Historical | — |
| [2026-08-10-viewer-video-preview-design.md](superpowers/specs/2026-08-10-viewer-video-preview-design.md) | Historical | — |
| [2026-08-13-viewer-video-control-visibility-design.md](superpowers/specs/2026-08-13-viewer-video-control-visibility-design.md) | Historical | — |
| [2026-08-13-viewer-video-player-experience-redesign.md](superpowers/specs/2026-08-13-viewer-video-player-experience-redesign.md) | Historical | — |
| [2026-08-14-video-interaction-and-aperture-design.md](superpowers/specs/2026-08-14-video-interaction-and-aperture-design.md) | Historical | — |
| [2026-08-14-viewer-video-interaction-performance-design.md](superpowers/specs/2026-08-14-viewer-video-interaction-performance-design.md) | Superseded | [2026-08-14-video-interaction-and-aperture-design.md](superpowers/specs/2026-08-14-video-interaction-and-aperture-design.md) |
| [2026-08-16-viewer-native-video-theater-refactor-design.md](superpowers/specs/2026-08-16-viewer-native-video-theater-refactor-design.md) | Historical | — |
| [2026-08-17-video-preview-compact-native-aspect-design.md](superpowers/specs/2026-08-17-video-preview-compact-native-aspect-design.md) | Historical | — |
| [2026-08-23-viewer-product-documentation-reconstruction-design.md](superpowers/specs/2026-08-23-viewer-product-documentation-reconstruction-design.md) | Historical | — |

### 实施计划

| Document | Status | Replaced by |
| --- | --- | --- |
| [2026-07-16-viewer-0.1-roadmap.md](superpowers/plans/2026-07-16-viewer-0.1-roadmap.md) | Historical | — |
| [2026-07-16-viewer-foundation-hardening-plan.md](superpowers/plans/2026-07-16-viewer-foundation-hardening-plan.md) | Historical | — |
| [2026-07-16-viewer-foundation-plan.md](superpowers/plans/2026-07-16-viewer-foundation-plan.md) | Historical | — |
| [2026-07-16-viewer-g1-image-pipeline-plan.md](superpowers/plans/2026-07-16-viewer-g1-image-pipeline-plan.md) | Historical | — |
| [2026-07-16-viewer-g2-file-transactions-plan.md](superpowers/plans/2026-07-16-viewer-g2-file-transactions-plan.md) | Historical | — |
| [2026-07-16-viewer-g3-scan-search-plan.md](superpowers/plans/2026-07-16-viewer-g3-scan-search-plan.md) | Historical | — |
| [2026-07-16-viewer-g4-architecture-freeze-plan.md](superpowers/plans/2026-07-16-viewer-g4-architecture-freeze-plan.md) | Historical | — |
| [2026-07-16-viewer-m1-browsing-core-plan.md](superpowers/plans/2026-07-16-viewer-m1-browsing-core-plan.md) | Historical | — |
| [2026-07-16-viewer-m2-review-efficiency-plan.md](superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md) | Historical | — |
| [2026-07-16-viewer-m3-organization-comparison-plan.md](superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md) | Historical | — |
| [2026-07-20-viewer-internal-pointer-drag-plan.md](superpowers/plans/2026-07-20-viewer-internal-pointer-drag-plan.md) | Historical | — |
| [2026-07-20-viewer-m3-drag-session-rearchitecture-plan.md](superpowers/plans/2026-07-20-viewer-m3-drag-session-rearchitecture-plan.md) | Historical | — |
| [2026-07-20-viewer-marquee-selection-plan.md](superpowers/plans/2026-07-20-viewer-marquee-selection-plan.md) | Historical | — |
| [2026-07-22-viewer-radial-contextmenu-fallback.md](superpowers/plans/2026-07-22-viewer-radial-contextmenu-fallback.md) | Historical | — |
| [2026-07-22-viewer-simplified-workspace-radial-menu-plan.md](superpowers/plans/2026-07-22-viewer-simplified-workspace-radial-menu-plan.md) | Historical | — |
| [2026-07-23-viewer-continuous-quality-governance-plan.md](superpowers/plans/2026-07-23-viewer-continuous-quality-governance-plan.md) | Historical | — |
| [2026-07-23-viewer-engineering-baseline-ci-plan.md](superpowers/plans/2026-07-23-viewer-engineering-baseline-ci-plan.md) | Historical | — |
| [2026-07-23-viewer-engineering-optimization-roadmap.md](superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md) | Historical | — |
| [2026-07-23-viewer-folder-filmstrip-rows.md](superpowers/plans/2026-07-23-viewer-folder-filmstrip-rows.md) | Historical | — |
| [2026-07-23-viewer-governance-m4-removal-plan.md](superpowers/plans/2026-07-23-viewer-governance-m4-removal-plan.md) | Historical | — |
| [2026-07-23-viewer-infrastructure-decomposition-plan.md](superpowers/plans/2026-07-23-viewer-infrastructure-decomposition-plan.md) | Historical | — |
| [2026-07-23-viewer-react-decomposition-plan.md](superpowers/plans/2026-07-23-viewer-react-decomposition-plan.md) | Historical | — |
| [2026-07-23-viewer-tauri-runtime-decomposition-plan.md](superpowers/plans/2026-07-23-viewer-tauri-runtime-decomposition-plan.md) | Historical | — |
| [2026-07-27-viewer-aspect-aware-thumbnail-density.md](superpowers/plans/2026-07-27-viewer-aspect-aware-thumbnail-density.md) | Historical | — |
| [2026-07-27-viewer-light-preview.md](superpowers/plans/2026-07-27-viewer-light-preview.md) | Historical | — |
| [2026-07-27-viewer-readme.md](superpowers/plans/2026-07-27-viewer-readme.md) | Historical | — |
| [2026-07-27-viewer-toolbar-menu-buttons.md](superpowers/plans/2026-07-27-viewer-toolbar-menu-buttons.md) | Historical | — |
| [2026-07-27-viewer-unified-preview-theme.md](superpowers/plans/2026-07-27-viewer-unified-preview-theme.md) | Historical | — |
| [2026-07-28-viewer-adaptive-text-panel.md](superpowers/plans/2026-07-28-viewer-adaptive-text-panel.md) | Historical | — |
| [2026-07-28-viewer-development-launcher.md](superpowers/plans/2026-07-28-viewer-development-launcher.md) | Historical | — |
| [2026-07-28-viewer-intelligent-compare-layout.md](superpowers/plans/2026-07-28-viewer-intelligent-compare-layout.md) | Historical | — |
| [2026-07-29-viewer-other-files-and-split-text-preview.md](superpowers/plans/2026-07-29-viewer-other-files-and-split-text-preview.md) | Historical | — |
| [2026-07-30-viewer-complete-ui-visual-upgrade.md](superpowers/plans/2026-07-30-viewer-complete-ui-visual-upgrade.md) | Historical | — |
| [2026-07-30-viewer-visual-fidelity-correction.md](superpowers/plans/2026-07-30-viewer-visual-fidelity-correction.md) | Historical | — |
| [2026-07-31-viewer-complete-ui-atlas-completion.md](superpowers/plans/2026-07-31-viewer-complete-ui-atlas-completion.md) | Historical | — |
| [2026-07-31-viewer-visual-atlas-final-remediation.md](superpowers/plans/2026-07-31-viewer-visual-atlas-final-remediation.md) | Historical | — |
| [2026-08-02-viewer-atlas-to-product-complete-migration.md](superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md) | Historical | — |
| [2026-08-02-viewer-single-instance-development-launcher.md](superpowers/plans/2026-08-02-viewer-single-instance-development-launcher.md) | Historical | — |
| [2026-08-03-viewer-native-acceptance-controller-plan.md](superpowers/plans/2026-08-03-viewer-native-acceptance-controller-plan.md) | Historical | — |
| [2026-08-04-viewer-tiered-visual-acceptance.md](superpowers/plans/2026-08-04-viewer-tiered-visual-acceptance.md) | Historical | — |
| [2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress.md](superpowers/plans/2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress.md) | Historical | — |
| [2026-08-05-viewer-progressive-folder-loading.md](superpowers/plans/2026-08-05-viewer-progressive-folder-loading.md) | Historical | — |
| [2026-08-06-viewer-compare-original-images-and-eight-item-limit.md](superpowers/plans/2026-08-06-viewer-compare-original-images-and-eight-item-limit.md) | Historical | — |
| [2026-08-06-viewer-five-level-thumbnail-slider-implementation-plan.md](superpowers/plans/2026-08-06-viewer-five-level-thumbnail-slider-implementation-plan.md) | Historical | — |
| [2026-08-06-viewer-square-image-card-selection.md](superpowers/plans/2026-08-06-viewer-square-image-card-selection.md) | Historical | — |
| [2026-08-06-viewer-trackpad-zoom-and-image-magnifier.md](superpowers/plans/2026-08-06-viewer-trackpad-zoom-and-image-magnifier.md) | Historical | — |
| [2026-08-07-viewer-fit-relative-original-preview-revision.md](superpowers/plans/2026-08-07-viewer-fit-relative-original-preview-revision.md) | Historical | — |
| [2026-08-07-viewer-pointer-following-magnifier-revision.md](superpowers/plans/2026-08-07-viewer-pointer-following-magnifier-revision.md) | Historical | — |
| [2026-08-07-viewer-stable-progressive-preview-and-larger-magnifier.md](superpowers/plans/2026-08-07-viewer-stable-progressive-preview-and-larger-magnifier.md) | Historical | — |
| [2026-08-09-preview-original-single-reveal.md](superpowers/plans/2026-08-09-preview-original-single-reveal.md) | Historical | — |
| [2026-08-09-viewer-bottom-shelf-layout.md](superpowers/plans/2026-08-09-viewer-bottom-shelf-layout.md) | Historical | — |
| [2026-08-09-viewer-stable-preview-loading.md](superpowers/plans/2026-08-09-viewer-stable-preview-loading.md) | Historical | — |
| [2026-08-10-viewer-bundled-video-preview.md](superpowers/plans/2026-08-10-viewer-bundled-video-preview.md) | Historical | — |
| [2026-08-13-viewer-video-control-visibility.md](superpowers/plans/2026-08-13-viewer-video-control-visibility.md) | Historical | — |
| [2026-08-13-viewer-video-player-experience-redesign.md](superpowers/plans/2026-08-13-viewer-video-player-experience-redesign.md) | Historical | — |
| [2026-08-14-video-interaction-and-aperture.md](superpowers/plans/2026-08-14-video-interaction-and-aperture.md) | Historical | — |
| [2026-08-14-viewer-video-interaction-performance.md](superpowers/plans/2026-08-14-viewer-video-interaction-performance.md) | Historical | — |
| [2026-08-16-viewer-native-video-theater-refactor.md](superpowers/plans/2026-08-16-viewer-native-video-theater-refactor.md) | Historical | — |
| [2026-08-17-video-preview-compact-native-aspect.md](superpowers/plans/2026-08-17-video-preview-compact-native-aspect.md) | Historical | — |
| [2026-08-17-video-rate-menu.md](superpowers/plans/2026-08-17-video-rate-menu.md) | Historical | — |
| [2026-08-18-video-player-faithful-layout.md](superpowers/plans/2026-08-18-video-player-faithful-layout.md) | Historical | — |
| [2026-08-24-viewer-product-documentation-reconstruction.md](superpowers/plans/2026-08-24-viewer-product-documentation-reconstruction.md) | Historical | — |

### 评审与验收证据

| Document | Status | Replaced by |
| --- | --- | --- |
| [2026-07-16-g1-image-pipeline-review.md](reviews/2026-07-16-g1-image-pipeline-review.md) | Development evidence | — |
| [2026-07-16-g2-file-transactions-review.md](reviews/2026-07-16-g2-file-transactions-review.md) | Development evidence | — |
| [2026-07-16-g3-scan-search-review.md](reviews/2026-07-16-g3-scan-search-review.md) | Development evidence | — |
| [2026-07-16-g4-architecture-freeze-review.md](reviews/2026-07-16-g4-architecture-freeze-review.md) | Development evidence | — |
| [2026-07-16-m1-browsing-core-review.md](reviews/2026-07-16-m1-browsing-core-review.md) | Development evidence | — |
| [2026-07-16-m2-review-efficiency-review.md](reviews/2026-07-16-m2-review-efficiency-review.md) | Development evidence | — |
| [2026-07-16-m3-organization-comparison-review.md](reviews/2026-07-16-m3-organization-comparison-review.md) | Development evidence | — |
| [2026-07-30-viewer-ui-visual-upgrade-verification.md](reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md) | Development evidence | — |
| [2026-08-02-viewer-atlas-product-component-gap-audit.md](reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md) | Development evidence | — |
| [2026-08-02-viewer-atlas-product-migration-ledger.md](reviews/2026-08-02-viewer-atlas-product-migration-ledger.md) | Development evidence | — |
| [2026-08-26-image-review-visual-baseline.md](reviews/2026-08-26-image-review-visual-baseline.md) | Development evidence | — |
| [2026-08-27-continuous-review-domain-phase-a.md](reviews/2026-08-27-continuous-review-domain-phase-a.md) | Development evidence | — |
| [2026-08-27-continuous-review-persistence-phase-b.md](reviews/2026-08-27-continuous-review-persistence-phase-b.md) | Development evidence | — |
| [2026-08-27-continuous-review-application-phase-c.md](reviews/2026-08-27-continuous-review-application-phase-c.md) | Development evidence | — |
| [2026-08-05-viewer-progressive-loading-acceptance.md](reviews/2026-08-05-viewer-progressive-loading-acceptance.md) | Development evidence | — |
| [2026-08-10-video-preview-acceptance.md](reviews/2026-08-10-video-preview-acceptance.md) | Development evidence | — |
| [2026-08-10-video-render-feasibility.md](reviews/2026-08-10-video-render-feasibility.md) | Development evidence | — |
| [2026-08-16-viewer-native-video-theater-visual-contract.md](reviews/2026-08-16-viewer-native-video-theater-visual-contract.md) | Development evidence | — |
| [viewer-native-smoke-matrix.md](reviews/viewer-native-smoke-matrix.md) | Development evidence | — |
| [acceptance/m3-organization-comparison-acceptance.md](acceptance/m3-organization-comparison-acceptance.md) | Development evidence | — |
| [progress/2026-07-16-viewer-m3-paused-checkpoint.md](progress/2026-07-16-viewer-m3-paused-checkpoint.md) | Historical | — |
| [progress/2026-08-03-viewer-atlas-product-migration-paused-checkpoint.md](progress/2026-08-03-viewer-atlas-product-migration-paused-checkpoint.md) | Historical | — |
| [progress/2026-08-26-image-annotation-review-workbench-checkpoint.md](progress/2026-08-26-image-annotation-review-workbench-checkpoint.md) | Development evidence | — |
| [progress/2026-08-27-continuous-review-phase-d-architecture-checkpoint.md](progress/2026-08-27-continuous-review-phase-d-architecture-checkpoint.md) | Development evidence | — |
| [prototypes/viewer-complete-ui-visual-atlas.html](prototypes/viewer-complete-ui-visual-atlas.html) | Historical | — |

## 状态定义

- **Current**：与当前开发版本同步的产品/用户文档。
- **Active**：仍在约束或执行当前工程工作的研究、架构决策、设计或计划。
- **Historical**：保留用于追溯的目标、设计、计划、进度或原型，不作为当前产品事实源。
- **Superseded**：已被“Replaced by”列中的新决策或文档明确取代。
- **Development evidence**：某次开发基线的评审或验收证据；证明当时被测范围，不承诺正式发布。

# Viewer UI Visual Upgrade Verification

> 最终复核：2026-08-05
> 结论：macOS 当前开发阶段的完整视觉升级与分层验收已完成；P0/P1/P2 未关闭项为 `0`。

## 1. 当前证据权威

- 最终视觉规范：`docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- 完整迁移设计：`docs/superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md`
- 非遗漏审计：`docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- 逐项迁移台账：`docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- 分层验收计划：`docs/superpowers/plans/2026-08-04-viewer-tiered-visual-acceptance.md`
- 正式产品证据提交：`9114b78515baa069914a6f6dc083a05a9982e9cf`
- 原生验收控制器提交：`7fa4005f2d9baab6805264ecbef5d51a0b429de3`
- 分支：`codex/viewer-atlas-product-migration`
- 当前平台：macOS；Windows 原生视觉验收留待 Windows 版本开发阶段。

后两个提交只修正非发布验收控制器的窗口交接和目标定位，没有改变正式产品组件，因此浏览器全量证据保留在正式产品证据提交，原生旅程证据记录在最终控制器提交。

## 2. 自动化门禁

| 命令 | 结果 | 证据摘要 |
| --- | --- | --- |
| `pnpm --dir ui check` | 通过 | Biome 与 TypeScript 检查通过；仅保留已有 Biome 配置弃用提示。 |
| `pnpm --dir ui test` | 通过 | 77 个测试文件；727 项通过，1 项按预期跳过。 |
| `pnpm --dir ui build` | 通过 | TypeScript 与 Vite 正式生产构建通过。 |
| `pnpm test:visual-acceptance` | 通过 | 16 项全量视觉控制器测试通过。 |
| `pnpm test:native-acceptance` | 通过 | 99 项 PID、进程、窗口、路径、协议、夹具、manifest 与 PNG 控制器测试通过。 |

## 3. 浏览器完整视觉验收

- 范围：17 组、89 个产品状态。
- 尺寸：`1024 × 720` 与 `1440 × 900`。
- 结果：89 × 2，共 178 组参考/产品联合图全部通过；失败 `0`，未运行 `0`。
- 运行质量：控制台错误 `0`，页面错误 `0`。
- 证据根目录：`target/viewer-visual-acceptance/final-all-9114b78/9114b78515baa069914a6f6dc083a05a9982e9cf/`
- 总览联系表：`target/viewer-visual-acceptance/final-all-9114b78/contact-sheets/`

四个波次与两个尺寸的联系表均已人工检查。主框架、筛选、菜单、圆盘、预览、对比、文本、信息、全部对话框、任务/结果、恢复与可访问性状态未发现剩余 P0/P1/P2 视觉问题。

## 4. macOS 原生代表旅程

原生层用于验证浏览器无法证明的窗口归属、系统标题栏、焦点交接、真实输入、弹层/圆盘打开路径和原生截图链路。它不重复机械执行全部 89 个静态状态。

| 窗口尺寸 | 结果 | 证据根目录 |
| --- | --- | --- |
| `1024 × 720` | 15/15 通过 | `target/atlas-product-migration-acceptance/7fa4005f2d9baab6805264ecbef5d51a0b429de3/1024x720/native-smoke/` |
| `1440 × 900` | 15/15 通过 | `target/atlas-product-migration-acceptance/7fa4005f2d9baab6805264ecbef5d51a0b429de3/1440x900/native-smoke/` |

两轮均绑定当前工作树的唯一裸开发进程 `target/debug/viewer-desktop`；未使用注册 bundle 或其他工作树进程作为验收目标。

## 5. 最终复核中修正的问题

- 项目结构根视图与聚合提示曾形成错误的标题/全宽带，已恢复为连续文件夹带和紧凑状态标签。
- 高级筛选在紧凑窗口下底部摘要与完成动作不可达，已增加受控滚动区并保持弹层锚定。
- 原生验收控制器曾因信息检查器残留、Finder 交接后未重新聚焦 Viewer、重命名标题目标过时而误报失败；这些问题只影响验收自动化，均已增加回归测试并修正。

## 6. 完成结论

正式产品代码已覆盖逐项审计中的 17 组、89 个状态；共享 primitives、全部界面模块和组件状态已经回写。完整浏览器联合对照与双尺寸 macOS 原生代表旅程均通过，当前阶段 P0/P1/P2 为 `0`。

Windows 系统标题栏、字体栅格、系统高对比度、输入设备和窗口行为仍必须在 Windows 版本开始开发后以原生环境重新验收；这是一项明确的平台待办，不是当前 macOS 视觉升级的未完成项。

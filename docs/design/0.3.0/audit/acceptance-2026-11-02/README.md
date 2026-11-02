# 0.3.0 全量验收快照（开发版）

- 日期：2026-11-02
- 范围：`acceptanceStateCatalog.json` 的全部 147 个状态
- 视口：`1440x900`、`1024x720`
- 结果：147 × 2 个 `product.png` 全部生成，`data-acceptance-status="ready"`，无缺失状态
- 联系方式：4 张 contact sheet；未逐张做像素 diff

## 文件

- `contact-1440x900-1.png` / `contact-1440x900-2.png`
- `contact-1024x720-1.png` / `contact-1024x720-2.png`
- 关键修复状态原图：
  - [RVW-19 矩形选中/手柄](RVW-19-1440x900.png)
  - [RVW-20 画笔重绘后保持工具](RVW-20-1440x900.png)
  - [RVW-23 返回评审素材网格](RVW-23-1440x900.png)
  - [RVW-24 200% 浏览器缩放紧凑工具栏](RVW-24-1440x900.png)
  - [SID-02 文件夹栏拖拽宽度](SID-02-1440x900.png)

## 参考图缺口

本次运行只缺 `target/atlas-product-migration-reference/.../reference.png`，因此
`viewer-visual-acceptance.mjs` 在复制参考图阶段返回退出码 1；产品截图、场景 ready、
尺寸/溢出指标均已完成。未把缺失的旧参考图计作产品失败，也未声称逐像素回归通过。

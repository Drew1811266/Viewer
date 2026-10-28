from pathlib import Path
from textwrap import dedent

ROOT = Path(__file__).parent
SPRITE = (ROOT / "assets" / "icons.svg").read_text()

def icon(name, cls=""):
    return f'<svg class="icon {cls}" aria-hidden="true"><use href="#i-{name}"/></svg>'

def topbar(crumb, actions="", search=True):
    search_html = ""
    if search:
        search_html = f'''
        <div class="searchbar" role="search">
          {icon("search")}
          <span>搜索素材、文件夹或意见</span>
          <span class="kbd">⌘K</span>
        </div>'''
    return f'''
    <header class="topbar">
      <div class="brand" aria-hidden="true">V</div>
      <div class="app-name">Viewer</div>
      <div class="breadcrumb">{crumb}</div>
      {search_html}
      <div class="top-actions">{actions}</div>
    </header>'''

def rail(active="assets"):
    items = [
        ("folder_open", "项目", "project"),
        ("grid_view", "素材", "assets"),
        ("comment", "评审", "review"),
        ("history", "历史", "history"),
    ]
    h = '<nav class="rail" aria-label="主导航">'
    for name, label, key in items:
        cls = "rail-item active" if key == active else "rail-item"
        h += f'<div class="{cls}">{icon(name)}<span>{label}</span></div>'
    h += '<div class="rail-spacer"></div>'
    h += f'<div class="rail-item">{icon("settings")}<span>设置</span></div>'
    h += f'<div class="rail-item">{icon("help")}<span>帮助</span></div>'
    h += '</nav>'
    return h

def pane():
    return f'''
    <aside class="pane" aria-label="文件夹">
      <div class="pane-head">
        <div class="pane-title">测试图</div>
        <button class="icon-btn icon-btn-sm" aria-label="新建文件夹">{icon("create_new_folder")}</button>
      </div>
      <div class="pane-body">
        <div class="tree-row"><span>{icon("grid_view")}</span><span>全部素材</span><span class="count">42</span></div>
        <div class="tree-row active"><span>{icon("image")}</span><span>截图</span><span class="count">28</span></div>
        <div class="tree-row child"><span>{icon("folder")}</span><span>桌面端</span><span class="count">16</span></div>
        <div class="tree-row child"><span>{icon("folder")}</span><span>移动端</span><span class="count">12</span></div>
        <div class="tree-row"><span>{icon("folder")}</span><span>设计稿</span><span class="count">9</span></div>
        <div class="tree-row"><span>{icon("movie")}</span><span>视频</span><span class="count">3</span></div>
        <div class="tree-row"><span>{icon("description")}</span><span>其他</span><span class="count">2</span></div>
        <div class="tree-section">评审状态</div>
        <div class="tree-row"><span>{icon("pending")}</span><span>待评审</span><span class="count">12</span></div>
        <div class="tree-row"><span>{icon("progress_activity")}</span><span>进行中</span><span class="count">3</span></div>
        <div class="tree-row"><span>{icon("check_circle")}</span><span>已完成</span><span class="count">27</span></div>
      </div>
    </aside>'''

def shot(v=0):
    # deterministic tiny dashboard composition
    positions = [
        (27, 28, 24, 16), (53, 28, 18, 16), (27, 48, 44, 28),
        (27, 80, 13, 12), (42, 80, 13, 12), (57, 80, 13, 12),
    ]
    x, y, w, h = positions[v % len(positions)]
    bars = "".join(
        f'<span class="s-bar" style="left:{31+i*6}%;height:{10+((v*7+i*13)%22)}%"></span>'
        for i in range(6)
    )
    return f'''
    <div class="shot">
      <div class="s-top"></div>
      <div class="s-title" style="top:{y}%;left:{x}%;width:{w}%"></div>
      <div class="s-line" style="top:{y+9}%;left:{x}%;width:{w+18}%"></div>
      <div class="s-card" style="top:{y+18}%;left:{x}%;width:{w}%;height:{h}%"></div>
      <div class="s-accent" style="top:{y+18}%;left:{x+w+4}%;width:{18}%;height:{h}%"></div>
      <div class="s-chart" style="top:{y+18+h+4}%;left:{x}%;width:{w+22}%;height:{34-h}%"></div>
      {bars}
      <span class="s-dot" style="left:{x+w+12}%;top:{y+6}%"></span>
      <span class="s-dot" style="left:{x+w+16}%;top:{y+6}%;background:#7fb7b1"></span>
    </div>'''

def asset_card(name, meta, status, v, selected=False, status_cls="status-muted"):
    sel = " selected" if selected else ""
    return f'''
    <article class="card{sel}">
      <div class="card-media">
        {shot(v)}
        <div class="thumb-label">{icon("image", "icon-sm")} PNG</div>
        {f'<div class="thumb-check">{icon("check", "")}</div>' if selected else ''}
      </div>
      <div class="card-info">
        <div style="min-width:0">
          <div class="card-name">{name}</div>
          <div class="card-meta"><span class="chip {status_cls}" style="height:20px;padding:0 7px;font-size:10px">{status}</span><span>{meta}</span></div>
        </div>
        <span class="card-more">{icon("more_horiz", "icon-sm")}</span>
      </div>
    </article>'''

def screen(id, title, body, cls=""):
    return f'''
    <section class="screen {cls}" id="{id}" data-title="{title}" hidden>
      {body}
    </section>'''

def tool_card(num, name, key, desc, demo):
    return f'''<article class="tool-card">
      <div class="tool-demo">{demo}</div>
      <div class="tool-meta"><div class="row"><span class="tool-num">{num}</span><strong>{name}</strong><span class="tool-key">{key}</span></div><p>{desc}</p></div>
    </article>'''

def tool_sheet():
    cards = [
        ("01", "选择", "V", "选中、移动、缩放；点选定位，方向键微调", f'<div class="center" style="height:100%">{icon("near_me")}<span class="small muted" style="margin-left:8px">点选移动</span></div>'),
        ("02", "图钉", "P", "指出具体位置；编号与右侧意见一一对应", '<div class="marker" style="left:calc(50% - 12px);top:calc(50% - 12px)">1</div>'),
        ("03", "箭头", "A", "从结论指向目标；可关闭箭头头部当直线用", f'<svg style="position:absolute;inset:18px 20px" viewBox="0 0 100 60" preserveAspectRatio="none"><path d="M4,54 C30,42 55,22 92,8" stroke="#d94f45" stroke-width="2" fill="none"/><polygon points="92,8 78,10 87,21" fill="#d94f45"/></svg>'),
        ("04", "直线", "L", "下划线、分隔、不抢视线的指向", '<div class="demo-line"></div>'),
        ("05", "画笔", "B", "手绘圈画，适合快速、不规则的强调", f'<svg style="position:absolute;inset:14px 18px" viewBox="0 0 100 70" preserveAspectRatio="none"><path d="M6,55 C22,18 34,66 48,34 S72,18 94,32" stroke="#d94f45" stroke-width="2.5" fill="none" stroke-linecap="round"/></svg>'),
        ("06", "高亮", "H", "半透明覆盖，不遮住底层文字与图像", '<div class="demo-hl"></div>'),
        ("07", "矩形", "R", "圈出卡片、按钮、区块；可拖动四角缩放", '<div class="demo-rect"></div>'),
        ("08", "椭圆", "O", "圈出圆形控件、头像或不规则区域", '<div class="demo-ellipse"></div>'),
        ("09", "文字", "T", "在画布上直接写短结论；双击编辑", '<div class="demo-text">标题层级不足</div>'),
        ("10", "多边形", "G", "贴合不规则浮层、长列表、错位元素", f'<svg style="position:absolute;inset:12px 20px" viewBox="0 0 100 70"><polygon points="24,58 38,12 76,8 92,36 66,62" fill="rgba(217,79,69,.08)" stroke="#d94f45" stroke-width="2"/></svg>'),
        ("11", "标尺", "M", "量间距、尺寸；Shift 约束水平/垂直", '<div class="demo-measure"><span>24 px</span></div>'),
    ]
    body = "".join(tool_card(*c) for c in cards)
    return screen("tool-gallery", "标注工具一览", f'''
      {topbar('<span>设计规范</span><span>/</span><strong>标注工具</strong>', f'<button class="btn btn-tonal btn-sm">{icon("grid_view")}设计系统</button>', search=False)}
      <main class="spec">
        <div class="spec-head">
          <div><div class="spec-eyebrow">REVIEW · MARKUP</div><h1 class="spec-title">标注工具</h1>
          <p class="spec-sub">10 种标记 + 选择。首屏 7 个常驻，长按展开直线、高亮、多边形等变体。</p></div>
          <div class="spec-actions"><span class="chip selected">浅色</span><span class="chip">桌面 −1</span></div>
        </div>
        <div class="tool-grid">{body}</div>
        <p class="small muted" style="margin-top:16px">所有工具共用 8 色 / 3 档线宽 / 透明度；直线与箭头可切换虚线。测量只作为某条意见的目标，不单独产生意见。</p>
      </main>
    ''')

# ---------------- 01 welcome ----------------
welcome = screen("welcome", "项目选择", f'''
  <div class="welcome" style="position:absolute;inset:0">
    <div class="welcome-top">
      <div class="brand">V</div><div class="app-name">Viewer</div>
      <div class="top-actions">
        <button class="icon-btn" aria-label="帮助">{icon("help")}</button>
        <button class="icon-btn" aria-label="设置">{icon("settings")}</button>
      </div>
    </div>
    <main class="welcome-main">
      <div class="welcome-brand"><div class="brand">V</div></div>
      <h1 class="welcome-title">打开一个项目，开始评审</h1>
      <p class="welcome-sub">本地文件夹就是项目。素材、意见和历史都留在你的 Mac 上。</p>
      <div class="dropzone">
        {icon("folder_open", "icon-xl")}
        <div class="dropzone-title">拖入文件夹，或选择本地目录</div>
        <div class="dropzone-sub">支持图片、视频、Markdown、HTML、SVG 与 PDF 等常见素材</div>
        <div class="welcome-actions">
          <button class="btn btn-filled">{icon("folder_open")}打开文件夹</button>
          <button class="btn btn-text">最近打开</button>
        </div>
      </div>
      <div class="recent">
        <div class="recent-head"><div class="recent-title">最近项目</div><span class="small muted" style="margin-left:auto">清空</span></div>
        <div class="recent-list">
          <div class="recent-row">{icon("folder")}<div><div class="recent-name">测试图</div><div class="recent-path">~/Design/测试图</div></div><div class="recent-meta">42 个素材 · 今天 16:42</div><button class="icon-btn icon-btn-sm" aria-label="更多">{icon("more_horiz", "icon-sm")}</button></div>
          <div class="recent-row">{icon("folder")}<div><div class="recent-name">品牌改版 2026</div><div class="recent-path">~/Work/品牌改版</div></div><div class="recent-meta">118 个素材 · 昨天</div><button class="icon-btn icon-btn-sm" aria-label="更多">{icon("more_horiz", "icon-sm")}</button></div>
          <div class="recent-row">{icon("folder")}<div><div class="recent-name">移动端设计稿</div><div class="recent-path">~/Design/mobile</div></div><div class="recent-meta">36 个素材 · 10 月 24 日</div><button class="icon-btn icon-btn-sm" aria-label="更多">{icon("more_horiz", "icon-sm")}</button></div>
        </div>
      </div>
      <p class="small muted" style="margin-top:18px">不需要账号，不会上传文件。语音转写也完全在本地完成。</p>
    </main>
  </div>''')

# ---------------- 02 workspace grid ----------------
workspace_grid = screen("workspace-grid", "工作区 · 网格", f'''
  {topbar('<span>测试图</span><span>/</span><strong>截图</strong>', f'<button class="icon-btn" aria-label="通知">{icon("notifications")}</button><div class="avatar">D</div>')}
  <div class="app-body">
    {rail()}
    {pane()}
    <main class="main">
      <div class="content-head">
        <div><div class="content-title">截图</div><div class="content-sub">28 个素材 · 42 条当前意见 · 3 个进行中</div></div>
        <div class="content-actions">
          <div class="segmented" role="group" aria-label="视图">
            <button class="selected">{icon("grid_view")}网格</button>
            <button>{icon("list")}列表</button>
          </div>
          <button class="btn btn-outlined btn-sm">{icon("sort")}最近修改</button>
          <button class="btn btn-filled btn-sm">{icon("comment")}开始评审</button>
        </div>
      </div>
      <div class="content-toolbar">
        <button class="chip selected">全部 <span class="count">28</span></button>
        <button class="chip">待评审 <span class="count">12</span></button>
        <button class="chip">有意见 <span class="count">12</span></button>
        <button class="chip">已归档 <span class="count">4</span></button>
        <span style="flex:1"></span>
        <button class="btn btn-text btn-sm">{icon("select_all")}选择</button>
      </div>
      <div class="grid">
        {asset_card("登录页-桌面.png", "2400×1600 · 3 条", "进行中", 0, True, "status-warn")}
        {asset_card("数据看板.png", "2880×1800 · 6 条", "进行中", 1, False, "status-warn")}
        {asset_card("设置页-深色.png", "2400×1600 · 2 条", "待评审", 2)}
        {asset_card("移动端-首页.png", "1170×2532 · 0 条", "待评审", 3)}
        {asset_card("空状态.png", "2400×1600 · 1 条", "已完成", 4, False, "status-ok")}
        {asset_card("删除确认弹窗.png", "2400×1600 · 4 条", "已完成", 5, False, "status-ok")}
        {asset_card("支付流程-步骤2.png", "2400×1600 · 2 条", "待评审", 6)}
        {asset_card("列表-筛选.png", "2400×1600 · 3 条", "已完成", 7, False, "status-ok")}
        {asset_card("引导页-01.png", "1170×2532 · 1 条", "待评审", 8)}
        {asset_card("个人中心.png", "1170×2532 · 0 条", "待评审", 9)}
        {asset_card("搜索-无结果.png", "2400×1600 · 2 条", "进行中", 10, False, "status-warn")}
        {asset_card("通知中心.png", "2400×1600 · 1 条", "已完成", 11, False, "status-ok")}
      </div>
    </main>
  </div>''')

# ---------------- 03 workspace list ----------------
rows = [
    ("数据看板.png", "image", "PNG", "2880×1800", "进行中", "今天 16:42", "6 条"),
    ("登录页-桌面.png", "image", "PNG", "2400×1600", "进行中", "今天 15:08", "3 条"),
    ("设置页-深色.png", "image", "PNG", "2400×1600", "待评审", "今天 14:22", "—"),
    ("移动端-首页.png", "image", "PNG", "1170×2532", "待评审", "今天 11:05", "—"),
    ("空状态.png", "image", "PNG", "2400×1600", "已完成", "昨天 18:30", "1 条"),
    ("删除确认弹窗.png", "image", "PNG", "2400×1600", "已完成", "昨天 17:12", "4 条"),
    ("支付流程-步骤2.png", "image", "PNG", "2400×1600", "待评审", "昨天 16:40", "—"),
    ("列表-筛选.png", "image", "PNG", "2400×1600", "已完成", "10 月 26 日", "3 条"),
    ("产品演示.mp4", "movie", "MP4", "1920×1080", "已归档", "10 月 25 日", "2 条"),
    ("评审说明.md", "description", "Markdown", "12 KB", "—", "10 月 24 日", "—"),
]
items = ""
for i, (name, ic, typ, size, status, time, count) in enumerate(rows):
    scls = "status-ok" if status == "已完成" else ("status-warn" if status == "进行中" else "status-muted")
    items += f'''<div class="table-row">
      <div class="table-name">{icon(ic)}<span>{name}</span></div>
      <div class="muted">{typ}</div><div class="muted">{size}</div>
      <div><span class="chip {scls}" style="height:22px;padding:0 8px;font-size:10px">{status}</span></div>
      <div class="muted">{count} · {time}</div>
      <button class="icon-btn icon-btn-sm" aria-label="更多">{icon("more_horiz", "icon-sm")}</button>
    </div>'''
workspace_list = screen("workspace-list", "工作区 · 列表", f'''
  {topbar('<span>测试图</span><span>/</span><strong>全部素材</strong>', f'<button class="icon-btn" aria-label="通知">{icon("notifications")}</button><div class="avatar">D</div>')}
  <div class="app-body">
    {rail()}
    {pane()}
    <main class="main">
      <div class="content-head">
        <div><div class="content-title">全部素材</div><div class="content-sub">42 个素材 · 按最近修改排序</div></div>
        <div class="content-actions">
          <div class="segmented"><button>{icon("grid_view")}网格</button><button class="selected">{icon("list")}列表</button></div>
          <button class="btn btn-outlined btn-sm">{icon("filter_list")}筛选</button>
          <button class="btn btn-filled btn-sm">{icon("comment")}开始评审</button>
        </div>
      </div>
      <div class="content-toolbar">
        <button class="chip selected">有意见 12</button>
        <button class="chip">截图</button>
        <button class="chip">最近 7 天</button>
        <span style="flex:1"></span>
        <span class="small muted">已选择 0 项</span>
      </div>
      <div class="table">
        <div class="table-head"><div>名称</div><div>类型</div><div>尺寸</div><div>评审状态</div><div>意见 / 修改时间</div><div></div></div>
        {items}
      </div>
      <div class="statusbar"><span class="status-dot"></span><span>42 个素材</span><span>·</span><span>12 个有意见</span><span class="right"><span>⌘A 全选</span><span>拖拽可多选</span></span></div>
    </main>
  </div>''')

SCREENS = [welcome, workspace_grid, workspace_list]

# ---------------- 04 image preview ----------------
def canvas_mock():
    return f'''
    <div class="canvas-mock">
      <div class="m-top"></div><div class="m-side"></div>
      <div class="m-logo"></div>
      <div class="m-nav"></div><div class="m-nav2"></div>
      <div class="m-title"></div><div class="m-sub"></div>
      <div class="m-card" style="left:24%;top:29%;width:22%;height:17%"></div>
      <div class="m-card" style="left:49%;top:29%;width:22%;height:17%"></div>
      <div class="m-card" style="left:74%;top:29%;width:20%;height:17%"></div>
      <div class="m-chart" style="left:24%;top:50%;width:46%;height:36%"></div>
      <div class="m-accent" style="left:73%;top:50%;width:21%;height:15%"></div>
      <div class="m-line" style="left:24%;top:55%;width:22%"></div>
      <div class="m-line" style="left:24%;top:60%;width:18%"></div>
      <div class="m-line" style="left:24%;top:65%;width:25%"></div>
      <div class="m-bar" style="left:29%;height:12%"></div>
      <div class="m-bar" style="left:35%;height:20%"></div>
      <div class="m-bar" style="left:41%;height:15%"></div>
      <div class="m-bar" style="left:47%;height:26%"></div>
      <div class="m-bar" style="left:53%;height:22%"></div>
      <div class="m-bar" style="left:59%;height:30%"></div>
    </div>'''

image_preview = screen("preview-image", "图片预览", f'''
  {topbar('<span>测试图</span><span>/</span><span>截图</span><span>/</span><strong>数据看板.png</strong>', f'<button class="btn btn-tonal btn-sm">{icon("comment")}开始评审</button><button class="icon-btn" aria-label="更多">{icon("more_horiz")}</button>')}
  <div class="preview-body">
    <div class="preview-stage light">
      <div class="preview-canvas">{canvas_mock()}</div>
      <div class="preview-toolbar" role="toolbar" aria-label="缩放">
        <button class="icon-btn" aria-label="缩小">{icon("zoom_out")}</button>
        <span class="zoom-pill">100%</span>
        <button class="icon-btn" aria-label="放大">{icon("zoom_in")}</button>
        <span class="sep" style="width:1px;height:24px;background:var(--md-outline-variant);margin:0 4px"></span>
        <button class="icon-btn" aria-label="适应窗口">{icon("fit_screen")}</button>
        <button class="icon-btn" aria-label="全屏">{icon("fullscreen")}</button>
      </div>
    </div>
    <aside class="preview-side">
      <div class="side-title">文件信息</div>
      <div class="meta-row"><span class="label">文件名</span><span class="val">数据看板.png</span></div>
      <div class="meta-row"><span class="label">尺寸</span><span class="val">2880 × 1800</span></div>
      <div class="meta-row"><span class="label">大小</span><span class="val">2.4 MB</span></div>
      <div class="meta-row"><span class="label">修改时间</span><span class="val">今天 16:42</span></div>
      <div class="meta-row"><span class="label">素材版本</span><span class="val">v5 · 当前</span></div>
      <div class="side-title" style="margin-top:24px">当前评审</div>
      <div class="card" style="padding:12px">
        <div class="row gap-8"><span class="badge">进行中</span><span class="small muted">第 2 轮</span></div>
        <div style="font-size:18px;font-weight:700;margin-top:8px">6 条意见</div>
        <div class="small muted mt-4">2 条待处理 · 最后保存 17:24</div>
        <button class="btn btn-filled btn-sm" style="width:100%;margin-top:12px">{icon("edit")}继续评审</button>
      </div>
      <div class="side-title" style="margin-top:24px">意见摘要</div>
      <div class="feedback-list" style="padding:0;overflow:visible">
        <div class="fb-item" style="padding:8px 0"><div class="fb-num">1</div><div class="fb-body"><div class="fb-text">卡片间距不一致</div><div class="fb-sub">桌面端 · 待处理</div></div></div>
        <div class="fb-item" style="padding:8px 0"><div class="fb-num teal">2</div><div class="fb-body"><div class="fb-text">图表在深色下对比度不足</div><div class="fb-sub">桌面端 · 待处理</div></div></div>
      </div>
    </aside>
  </div>''')

# ---------------- 05 text preview ----------------
text_preview = screen("preview-text", "Markdown 预览", f'''
  {topbar('<span>测试图</span><span>/</span><span>文档</span><span>/</span><strong>评审说明.md</strong>', f'<div class="segmented"><button class="selected">预览</button><button>编辑</button></div><button class="icon-btn" aria-label="更多">{icon("more_horiz")}</button>')}
  <div class="preview-body">
    <aside class="pane" style="width:240px;min-width:240px">
      <div class="pane-head"><div class="pane-title">大纲</div></div>
      <div class="pane-body">
        <div class="tree-row active">评审范围</div>
        <div class="tree-row">视觉层级</div>
        <div class="tree-row">交互状态</div>
        <div class="tree-row">待确认问题</div>
        <div class="tree-row">验收清单</div>
      </div>
    </aside>
    <div class="doc-stage">
      <article class="doc-page">
        <h1>数据看板 · 第 2 轮评审说明</h1>
        <div class="meta">2026-10-28 · Drew · 基于素材版本 v5</div>
        <h2>评审范围</h2>
        <p>本轮聚焦桌面端数据看板的层级、密度与异常状态。移动端断点和图表交互不在本轮范围内。</p>
        <h2>视觉层级</h2>
        <ul>
          <li>主指标卡片的数字应成为第一视觉焦点，当前标题与数字的对比度不足。</li>
          <li>筛选器与时间范围属于同一组操作，建议合并为一个工具条。</li>
          <li>图表网格线过强，压低到 <code>outline-variant</code> 级别即可。</li>
        </ul>
        <h2>交互状态</h2>
        <p>空数据、加载失败、权限不足三种状态需要同时提供说明与下一步动作，不能只显示空白图表。</p>
        <h2>待确认问题</h2>
        <p>“导出”放在页面右上角还是图表卡片内？需要与产品确认后再定。</p>
        <div style="margin-top:28px;padding:12px;border-left:3px solid var(--md-primary);background:var(--md-surface-container-low);font-size:12px;line-height:19px">
          这条说明是项目内的普通 Markdown 文件，不参与评审协议，也不会被 Viewer 改写。
        </div>
      </article>
    </div>
    <aside class="preview-side" style="width:280px;min-width:280px">
      <div class="side-title">文档信息</div>
      <div class="meta-row"><span class="label">格式</span><span class="val">Markdown</span></div>
      <div class="meta-row"><span class="label">大小</span><span class="val">12 KB</span></div>
      <div class="meta-row"><span class="label">修改时间</span><span class="val">10 月 24 日</span></div>
      <div class="side-title" style="margin-top:24px">相关素材</div>
      <div class="recent-row" style="padding:8px 0;height:auto;border:0">{icon("image")}<div><div class="recent-name">数据看板.png</div><div class="recent-path">6 条意见</div></div></div>
      <div class="recent-row" style="padding:8px 0;height:auto;border:0">{icon("image")}<div><div class="recent-name">空状态.png</div><div class="recent-path">1 条意见</div></div></div>
      <button class="btn btn-outlined btn-sm" style="width:100%;margin-top:16px">{icon("open_in_new")}用默认应用打开</button>
    </aside>
  </div>''')

# ---------------- 06 video preview ----------------
video_preview = screen("preview-video", "视频预览", f'''
  {topbar('<span>测试图</span><span>/</span><span>视频</span><span>/</span><strong>产品演示.mp4</strong>', f'<button class="btn btn-tonal btn-sm">{icon("comment")}视频评审</button><button class="icon-btn" aria-label="更多">{icon("more_horiz")}</button>')}
  <div class="preview-body">
    <div class="preview-stage">
      <div class="video-frame">
        <div class="v-bg"></div>
        <div class="v-ui">{icon("play_arrow")}</div>
        <div class="video-controls">
          <div class="video-timeline"><span></span></div>
          <div class="row">{icon("pause")}<span>01:12 / 03:08</span><span style="flex:1"></span>{icon("volume_up")}<span>1.0×</span>{icon("fullscreen")}</div>
        </div>
      </div>
    </div>
    <aside class="preview-side">
      <div class="side-title">视频信息</div>
      <div class="meta-row"><span class="label">时长</span><span class="val">03:08</span></div>
      <div class="meta-row"><span class="label">分辨率</span><span class="val">1920 × 1080</span></div>
      <div class="meta-row"><span class="label">大小</span><span class="val">84 MB</span></div>
      <div class="meta-row"><span class="label">编码</span><span class="val">H.264 / AAC</span></div>
      <div class="side-title" style="margin-top:24px">评审片段</div>
      <div class="card" style="padding:12px">
        <div class="row gap-8"><span class="badge">2 条意见</span><span class="small muted">2 个时间点</span></div>
        <div class="mt-8 small">00:18 · 转场节奏偏快</div>
        <div class="mt-4 small">01:12 · 字幕遮挡按钮</div>
        <button class="btn btn-outlined btn-sm" style="width:100%;margin-top:12px">{icon("play_arrow")}从第一条意见播放</button>
      </div>
      <p class="small muted mt-16">视频评审使用时间点与时间范围标记；0.3.0 不扩展视频工具，只统一界面。</p>
    </aside>
  </div>''')

# ---------------- 07 other file ----------------
other_preview = screen("preview-other", "其他文件", f'''
  {topbar('<span>测试图</span><span>/</span><span>其他</span><span>/</span><strong>brand-guide.pdf</strong>', f'<button class="icon-btn" aria-label="更多">{icon("more_horiz")}</button>')}
  <div class="preview-body">
    <div class="preview-stage light">
      <div class="center stack" style="align-items:center;gap:10px;text-align:center">
        <div style="width:72px;height:72px;border-radius:20px;background:var(--md-surface-container-high);display:flex;align-items:center;justify-content:center;color:var(--md-on-surface-variant)">{icon("picture_as_pdf", "icon-xl")}</div>
        <div style="font-size:20px;font-weight:700">这个格式暂不支持预览</div>
        <div class="small muted" style="max-width:380px;line-height:18px">Viewer 仍会把它保留在项目中，并记录文件信息。你可以在默认应用里打开，或将它作为评审附件引用。</div>
        <div class="row gap-8 mt-8"><button class="btn btn-filled">{icon("open_in_new")}用默认应用打开</button><button class="btn btn-outlined">{icon("folder_open")}在 Finder 中显示</button></div>
      </div>
    </div>
    <aside class="preview-side">
      <div class="side-title">文件信息</div>
      <div class="meta-row"><span class="label">文件名</span><span class="val">brand-guide.pdf</span></div>
      <div class="meta-row"><span class="label">类型</span><span class="val">PDF 文档</span></div>
      <div class="meta-row"><span class="label">大小</span><span class="val">4.8 MB</span></div>
      <div class="meta-row"><span class="label">修改时间</span><span class="val">10 月 20 日</span></div>
      <div class="side-title" style="margin-top:24px">评审</div>
      <p class="small muted" style="line-height:18px">此格式不支持画布标注。你仍然可以在右侧记录整体意见，或把它加入包含多个素材的评审。</p>
      <button class="btn btn-outlined btn-sm" style="width:100%;margin-top:12px">{icon("comment")}记录整体意见</button>
    </aside>
  </div>''')

SCREENS += [image_preview, text_preview, video_preview, other_preview]

# ---------------- review helpers ----------------
def toolrail(active="pin", disabled=False):
    tools = [
        ("near_me", "选择", "V", "select"),
        ("location_on", "标记", "P", "pin"),
        ("line_axis", "线条", "L", "line"),
        ("gesture", "画笔", "B", "brush"),
        ("square", "形状", "R", "shape"),
        ("title", "文字", "T", "text"),
        ("straighten", "标尺", "M", "measure"),
    ]
    h = f'<nav class="toolrail" aria-label="标注工具"{" style=opacity:.38" if disabled else ""}>'
    for name, label, key, ident in tools:
        cls = "tool active" if ident == active else "tool"
        h += f'<button class="{cls}" aria-label="{label}" {"disabled" if disabled else ""}>{icon(name)}<span>{label}</span><span class="key">{key}</span><span class="caret">▾</span></button>'
    h += '<div class="tool-sep"></div>'
    h += f'<button class="tool" aria-label="撤销" {"disabled" if disabled else ""}>{icon("undo")}<span>撤销</span></button>'
    h += f'<button class="tool" aria-label="重做" {"disabled" if disabled else ""}>{icon("redo")}<span>重做</span></button>'
    h += '<div class="rail-spacer"></div>'
    h += f'<button class="tool" aria-label="快捷键">{icon("keyboard")}<span>快捷键</span></button>'
    h += '</nav>'
    return h

def feedback_rail(selected=2, compact=False):
    items = [
        (1, "卡片之间的间距不一致，桌面端看起来更明显。", "桌面端 · 待处理", "", "red"),
        (2, "图表在深色背景下对比度不足，网格线太抢。", "桌面端 · 待处理", "teal", "teal"),
        (3, "筛选器和时间范围应该合成一个工具条。", "桌面端 · 已确认", "", "red"),
        (4, "空数据状态缺少下一步动作。", "状态 · 待处理", "", "amber"),
        (5, "数字字号可以再大一级，当前层级不够。", "桌面端 · 已确认", "teal", "teal"),
        (6, "导出按钮的位置需要和产品确认。", "疑问 · 待确认", "", ""),
    ]
    h = f'''
    <aside class="feedback" aria-label="意见列表">
      <div class="feedback-head">
        <div class="feedback-title">{icon("comment", "icon-sm")} 意见 <span class="feedback-count">42</span><span style="margin-left:auto" class="small muted">当前轮</span></div>
        <div class="feedback-filter">
          <button class="chip selected" style="height:26px;font-size:11px">全部</button>
          <button class="chip" style="height:26px;font-size:11px">待处理 8</button>
          <button class="chip" style="height:26px;font-size:11px">疑问 3</button>
        </div>
      </div>
      <div class="feedback-list">'''
    for n, text, sub, numcls, dot in items:
        sel = " selected" if n == selected else ""
        h += f'''<div class="fb-item{sel}">
          <div class="fb-num {numcls}">{n}</div>
          <div class="fb-body"><div class="fb-text">{text}</div><div class="fb-sub"><span class="status-dot" style="background:{'#b86e00' if dot=='amber' else ('#007d7a' if dot=='teal' else '#d94f45')}"></span>{sub}</div></div>
          <div class="fb-tools"><button class="icon-btn icon-btn-sm" aria-label="更多">{icon("more_horiz", "icon-sm")}</button></div>
        </div>'''
    h += f'''</div>
      <div class="feedback-foot">
        <button class="btn btn-text btn-sm">{icon("add")}整体意见</button>
        <span class="grow"></span>
        <button class="icon-btn icon-btn-sm" aria-label="筛选">{icon("filter_list", "icon-sm")}</button>
        <button class="icon-btn icon-btn-sm" aria-label="排序">{icon("sort", "icon-sm")}</button>
      </div>
    </aside>'''
    return h

def ann_pin(left, top, n, cls=""):
    return f'<div class="marker {cls}" style="left:{left}%;top:{top}%">{n}</div>'

def ann_box(left, top, width, height, cls=""):
    return f'<div class="ann-box {cls}" style="left:{left}%;top:{top}%;width:{width}%;height:{height}%"></div>'

def ann_label(left, top, text, cls=""):
    return f'<div class="ann-label {cls}" style="left:{left}%;top:{top}%">{text}</div>'

def ann_arrow(left, top, width, height, color="#d94f45"):
    return f'''<svg class="ann-arrow" style="left:{left}%;top:{top}%;width:{width}%;height:{height}%" viewBox="0 0 100 100" preserveAspectRatio="none">
      <path d="M4,92 C28,74 58,42 92,12" stroke="{color}"/>
      <polygon points="92,12 78,14 88,26" fill="{color}"/>
    </svg>'''

def annotation_highlighter(left, top, width, height):
    return f'<div style="position:absolute;left:{left}%;top:{top}%;width:{width}%;height:{height}%;background:rgba(255,214,64,.45);border-radius:2px;z-index:2"></div>'

def stylebar():
    return f'''
    <div class="stylebar" role="toolbar" aria-label="标注样式">
      <button class="swatch red active" aria-label="红色"></button>
      <button class="swatch orange" aria-label="橙色"></button>
      <button class="swatch amber" aria-label="黄色"></button>
      <button class="swatch green" aria-label="绿色"></button>
      <button class="swatch blue" aria-label="蓝色"></button>
      <button class="swatch violet" aria-label="紫色"></button>
      <button class="swatch ink" aria-label="墨色"></button>
      <button class="swatch paper" aria-label="白色"></button>
      <span class="sep"></span>
      <button class="width-opt w1" aria-label="细线"><i></i></button>
      <button class="width-opt w2 selected" aria-label="常规线宽"><i></i></button>
      <button class="width-opt w3" aria-label="粗线"><i></i></button>
      <span class="sep"></span>
      <button class="icon-btn icon-btn-sm" aria-label="撤销">{icon("undo", "icon-sm")}</button>
      <button class="icon-btn icon-btn-sm" aria-label="重做">{icon("redo", "icon-sm")}</button>
    </div>'''

def composer(mode="text", text="这个卡片的间距和标题不一致，建议统一到 16px。", voice=False):
    mic_cls = "mic-btn recording" if voice else "mic-btn"
    voice_panel = ""
    if voice:
        bars = "".join(f'<i style="height:{8+((i*7)%20)}px"></i>' for i in range(28))
        voice_panel = f'''
        <div class="voice-panel">
          <span class="voice-dot"></span>
          <div class="waveform" aria-hidden="true">{bars}</div>
          <span class="voice-time">00:07</span>
        </div>'''
    foot = f'''
      <div class="composer-foot">
        <span class="composer-hint">{icon("lock", "icon-sm") if voice else icon("keyboard", "icon-sm")}{"本地转写 · 音频不会上传" if voice else "⌘↵ 保存 · Esc 取消"}</span>
        <button class="{mic_cls}" aria-label="{"停止录音" if voice else "语音输入"}">{icon("stop_circle" if voice else "mic")}</button>
        <button class="btn btn-text btn-sm">取消</button>
        <button class="btn btn-filled btn-sm">{"停止并转写" if voice else "保存"}</button>
      </div>'''
    return f'''
    <div class="composer" role="dialog" aria-label="标注意见">
      <div class="composer-head"><span class="composer-title">标注意见 · 2</span><span class="small muted">桌面端</span><button class="icon-btn icon-btn-sm composer-close" aria-label="关闭">{icon("close", "icon-sm")}</button></div>
      {voice_panel}
      <textarea aria-label="标注意见">{text}</textarea>
      {foot}
    </div>'''

def review_shell(active="pin", annotations="", style="", composer_html="", banner="", selected=2, disabled=False, extra="", status_right="100%"):
    return f'''
  <div class="review-shell">
    <header class="review-top">
      <button class="icon-btn icon-btn-sm" aria-label="返回">{icon("arrow_back", "icon-sm")}</button>
      <div><div class="review-title">数据看板.png</div><div class="review-meta">第 2 轮 · 42 条意见 · 自动保存于 17:24</div></div>
      <div class="top-actions">
        <button class="icon-btn" aria-label="缩放">{icon("zoom_in")}</button>
        <button class="icon-btn" aria-label="更多">{icon("more_horiz")}</button>
        <button class="btn btn-tonal btn-sm">完成评审</button>
      </div>
    </header>
    {banner}
    <div class="review-body">
      {toolrail(active, disabled)}
      <div class="review-stage">
        <div class="canvas-wrap">
          {canvas_mock()}
          {annotations}
        </div>
        {style}
        {composer_html}
      </div>
      {feedback_rail(selected)}
    </div>
    <div class="statusbar">
      <span class="status-dot"></span><span>已保存</span><span>·</span><span>第 2 轮</span><span>·</span><span>42 条意见</span>
      <span class="right"><span>画布 {status_right}</span><span>2880 × 1800</span><span>{icon("keyboard", "icon-sm")} 快捷键</span></span>
    </div>
    {extra}
  </div>'''

default_annotations = f'''
  {ann_box(49, 29, 22, 18)}
  {ann_box(24, 50, 46, 36)}
  {ann_pin(66, 30, 1, "")}
  {ann_pin(70, 52, 2, "teal")}
  {ann_arrow(46, 31, 18, 18)}
  {annotation_highlighter(73, 54, 19, 7)}
  {ann_label(71, 34, "对比度不足", "red")}
  {ann_label(25, 43, "把筛选器合并", "")}
'''

review_default = screen("review-default", "评审工作台 · 默认", review_shell(active="pin", annotations=default_annotations))

selected_annotations = f'''
  {ann_box(49, 29, 22, 18, "selected")}
  <span class="handle" style="left:48.6%;top:28.4%"></span><span class="handle" style="right:28.4%;top:28.4%"></span>
  <span class="handle" style="left:48.6%;bottom:52.4%"></span><span class="handle" style="right:28.4%;bottom:52.4%"></span>
  {ann_pin(66, 30, 1, "")}
  {ann_pin(70, 52, 2, "teal")}
  {ann_arrow(46, 31, 18, 18)}
  {annotation_highlighter(73, 54, 19, 7)}
  {ann_label(71, 34, "已选中：图表卡片", "")}
'''
review_selected = screen("review-selected", "评审工作台 · 选中与样式", review_shell(active="shape", annotations=selected_annotations, style=stylebar(), selected=2))

text_annotations = f'''
  {ann_box(49, 29, 22, 18)}
  {ann_pin(66, 30, 1, "")}
  {ann_pin(70, 52, 2, "teal")}
  <div class="ann-label" style="left:53%;top:63%;border-color:var(--md-primary);color:var(--md-on-surface);padding:8px 10px;font-weight:500;max-width:220px">这里的数字应该更大，和标题拉开层级。</div>
  <span style="position:absolute;left:53%;top:69%;width:2px;height:8%;background:var(--md-primary);z-index:4"></span>
'''
review_text = screen("review-text", "评审工作台 · 文字标注与语音入口", review_shell(
    active="text", annotations=text_annotations,
    composer_html=composer("text", "这里的数字应该更大，和标题拉开层级。", False), selected=5))

voice_annotations = f'''
  {ann_box(49, 29, 22, 18, "selected")}
  {ann_pin(66, 30, 1, "")}
  {ann_pin(70, 52, 2, "teal")}
  {ann_label(71, 34, "正在补充说明", "")}
'''
review_voice = screen("review-voice", "评审工作台 · 语音转写", review_shell(
    active="pin", annotations=voice_annotations,
    composer_html=composer("voice", "这个卡片的间距和标题不一致，建议统一到 16px，并且在窄窗口下也要保持……", True), selected=2))

readonly_banner = f'''
  <div class="banner">
    {icon("info")}
    <div><strong>此评审已归档，无法修改标记。</strong><span style="margin-left:8px;opacity:.8">归档于 10 月 27 日 · 第 2 轮 · 42 条意见只读</span></div>
    <button class="btn btn-filled btn-sm">创建新评审</button>
  </div>'''
review_readonly = screen("review-readonly", "评审工作台 · 只读解释", review_shell(
    active="pin", annotations=default_annotations, banner=readonly_banner, selected=2, disabled=True))


# voice permission error
voice_error_composer = f'''
    <div class="composer" role="dialog" aria-label="语音输入不可用">
      <div class="composer-head"><span class="composer-title">标注意见 · 2</span><span class="small muted">桌面端</span><button class="icon-btn icon-btn-sm composer-close" aria-label="关闭">{icon("close", "icon-sm")}</button></div>
      <div style="display:flex;gap:12;padding:6px 4px 12px;align-items:flex-start">
        <div style="width:40px;height:40px;border-radius:50%;background:var(--md-error-container);color:var(--md-on-error-container);display:flex;align-items:center;justify-content:center;flex:0 0 auto">{icon("mic_off")}</div>
        <div><div style="font-size:13px;font-weight:600">无法使用麦克风</div><div class="small muted" style="line-height:17px;margin-top:2px">Viewer 没有麦克风权限。请在系统设置中允许，或直接键入文字。</div></div>
      </div>
      <textarea aria-label="标注意见" placeholder="直接键入意见…"></textarea>
      <div class="composer-foot">
        <span class="composer-hint">{icon("lock", "icon-sm")}音频不会上传，也不会写入磁盘</span>
        <button class="btn btn-text btn-sm">手动输入</button>
        <button class="btn btn-filled btn-sm">打开系统设置</button>
      </div>
    </div>'''
review_voice_error = screen("review-voice-error", "评审工作台 · 麦克风权限", review_shell(
    active="pin", annotations=voice_annotations, composer_html=voice_error_composer, selected=2))

SCREENS += [review_default, review_selected, review_text, review_voice, review_voice_error, review_readonly]

def workspace_bg(title="截图", subtitle="28 个素材 · 42 条当前意见 · 3 个进行中", cards=None, selected=0):
    if cards is None:
        cards = [
            ("登录页-桌面.png", "2400×1600 · 3 条", "进行中", "status-warn"),
            ("数据看板.png", "2880×1800 · 6 条", "进行中", "status-warn"),
            ("设置页-深色.png", "2400×1600 · 2 条", "待评审", "status-muted"),
            ("移动端-首页.png", "1170×2532 · 0 条", "待评审", "status-muted"),
            ("空状态.png", "2400×1600 · 1 条", "已完成", "status-ok"),
            ("删除确认弹窗.png", "2400×1600 · 4 条", "已完成", "status-ok"),
            ("支付流程-步骤2.png", "2400×1600 · 2 条", "待评审", "status-muted"),
            ("列表-筛选.png", "2400×1600 · 3 条", "已完成", "status-ok"),
        ]
    card_html = "".join(asset_card(n, m, s, i, i == selected, sc) for i, (n, m, s, sc) in enumerate(cards))
    return f'''
  {topbar('<span>测试图</span><span>/</span><strong>截图</strong>', f'<button class="icon-btn" aria-label="通知">{icon("notifications")}</button><div class="avatar">D</div>')}
  <div class="app-body">
    {rail()}
    {pane()}
    <main class="main">
      <div class="content-head">
        <div><div class="content-title">{title}</div><div class="content-sub">{subtitle}</div></div>
        <div class="content-actions">
          <div class="segmented"><button class="selected">{icon("grid_view")}网格</button><button>{icon("list")}列表</button></div>
          <button class="btn btn-outlined btn-sm">{icon("sort")}最近修改</button>
          <button class="btn btn-filled btn-sm">{icon("comment")}开始评审</button>
        </div>
      </div>
      <div class="content-toolbar">
        <button class="chip selected">全部 28</button><button class="chip">待评审 12</button><button class="chip">有意见 12</button><button class="chip">已归档 4</button>
      </div>
      <div class="grid">{card_html}</div>
    </main>
  </div>'''

def dialog(title, body, actions, cls="", icon_name=None, close=True):
    icon_html = f'<div class="dialog-icon">{icon(icon_name)}</div>' if icon_name else ""
    close_html = f'<button class="icon-btn icon-btn-sm dialog-close" aria-label="关闭">{icon("close", "icon-sm")}</button>' if close else ""
    return f'''
    <div class="scrim">
      <div class="dialog {cls}" role="dialog" aria-modal="true" aria-label="{title}">
        {close_html}
        {icon_html}
        <div class="dialog-title">{title}</div>
        <div class="dialog-body">{body}</div>
        <div class="dialog-actions">{actions}</div>
      </div>
    </div>'''

review_start = screen("review-start", "开始评审对话框", workspace_bg(cards=[
    ("登录页-桌面.png", "2400×1600 · 3 条", "进行中", "status-warn"),
    ("数据看板.png", "2880×1800 · 6 条", "进行中", "status-warn"),
    ("设置页-深色.png", "2400×1600 · 2 条", "待评审", "status-muted"),
    ("移动端-首页.png", "1170×2532 · 0 条", "待评审", "status-muted"),
], selected=1) + dialog(
    "开始第 3 轮评审",
    f'''
    <p style="margin:0 0 16px">将在当前素材版本 v5 上创建新一轮评审。上一轮的 42 条意见会保留在历史中，可以随时对照。</p>
    <div class="field"><label>评审名称</label><div class="value">数据看板 · 第 3 轮</div></div>
    <div class="row gap-8 mt-8"><span class="chip selected">{icon("image")}当前素材</span><span class="chip">包含 1 个素材</span></div>
    <p class="small muted mt-12" style="margin-bottom:0">开始后会自动保存到项目内的评审记录，不需要手动导出。</p>
    ''',
    '<button class="btn btn-text">取消</button><button class="btn btn-filled">开始评审</button>',
    cls="", icon_name="comment"))

review_complete = screen("review-complete", "完成评审对话框", workspace_bg(cards=[
    ("数据看板.png", "2880×1800 · 6 条", "进行中", "status-warn"),
    ("登录页-桌面.png", "2400×1600 · 3 条", "进行中", "status-warn"),
    ("空状态.png", "2400×1600 · 1 条", "已完成", "status-ok"),
    ("删除确认弹窗.png", "2400×1600 · 4 条", "已完成", "status-ok"),
], selected=0) + dialog(
    "完成第 2 轮评审？",
    f'''
    <p style="margin:0 0 16px">完成后这一轮会变为只读归档，仍可查看和导出；新意见请创建下一轮。</p>
    <div class="detail-stats" style="margin:0 0 12px">
      <div class="stat"><div class="n">42</div><div class="l">条意见</div></div>
      <div class="stat"><div class="n">8</div><div class="l">条待处理</div></div>
      <div class="stat"><div class="n">3</div><div class="l">条疑问</div></div>
    </div>
    <div class="row gap-8" style="font-size:13px"><span class="check-box checked">{icon("check", "icon-sm")}</span><span>同时生成 Markdown 评审摘要</span></div>
    <p class="small muted mt-8" style="margin-bottom:0">还有 8 条待处理意见。你可以继续评审，或先归档当前结果。</p>
    ''',
    '<button class="btn btn-text">继续评审</button><button class="btn btn-filled">完成并归档</button>',
    icon_name="archive"))

# ---------------- 15 history ----------------
history = screen("history", "评审历史", f'''
  {topbar('<span>测试图</span><span>/</span><strong>评审历史</strong>', f'<button class="btn btn-outlined btn-sm">{icon("download")}导出</button><div class="avatar">D</div>', search=False)}
  <div class="app-body">
    {rail("history")}
    <main class="main">
      <div class="content-head">
        <div><div class="content-title">评审历史</div><div class="content-sub">数据看板.png · 3 轮评审 · 最近更新今天 17:24</div></div>
        <div class="content-actions"><button class="btn btn-filled btn-sm">{icon("comment")}开始新评审</button></div>
      </div>
      <div class="history-layout">
        <div class="timeline">
          <div class="tl-item"><span class="tl-dot"></span><div><div class="tl-title">第 2 轮 · 进行中</div><div class="tl-sub">今天 17:24 · 42 条意见 · Drew</div></div></div>
          <div class="tl-item"><span class="tl-dot muted"></span><div><div class="tl-title">第 1 轮 · 已归档</div><div class="tl-sub">10 月 24 日 · 18 条意见 · Drew</div></div></div>
          <div class="tl-item"><span class="tl-dot muted"></span><div><div class="tl-title">外部修改 · v5</div><div class="tl-sub">10 月 24 日 14:02 · 素材版本变化</div></div></div>
          <div class="tl-item"><span class="tl-dot muted"></span><div><div class="tl-title">项目创建</div><div class="tl-sub">10 月 20 日 · 导入 42 个素材</div></div></div>
        </div>
        <div class="detail-card">
          <div class="row"><div><h3>第 2 轮评审</h3><div class="sub">基于素材版本 v5 · 开始于今天 15:08</div></div><span class="badge" style="margin-left:auto">进行中</span></div>
          <div class="detail-stats">
            <div class="stat"><div class="n">42</div><div class="l">当前意见</div></div>
            <div class="stat"><div class="n">8</div><div class="l">待处理</div></div>
            <div class="stat"><div class="n">3</div><div class="l">疑问</div></div>
          </div>
          <div class="side-title">意见分布</div>
          <div class="table" style="margin:0;border-radius:8px">
            <div class="table-head" style="grid-template-columns:2fr 1fr 1fr"><div>类型</div><div>数量</div><div>状态</div></div>
            <div class="table-row" style="grid-template-columns:2fr 1fr 1fr;height:44px"><div>布局与间距</div><div>18</div><div><span class="chip status-warn" style="height:20px;font-size:10px">处理中</span></div></div>
            <div class="table-row" style="grid-template-columns:2fr 1fr 1fr;height:44px"><div>视觉层级</div><div>12</div><div><span class="chip status-ok" style="height:20px;font-size:10px">已确认</span></div></div>
            <div class="table-row" style="grid-template-columns:2fr 1fr 1fr;height:44px"><div>交互状态</div><div>9</div><div><span class="chip status-warn" style="height:20px;font-size:10px">待处理</span></div></div>
            <div class="table-row" style="grid-template-columns:2fr 1fr 1fr;height:44px"><div>待确认问题</div><div>3</div><div><span class="chip status-muted" style="height:20px;font-size:10px">疑问</span></div></div>
          </div>
          <div class="row gap-8 mt-16"><button class="btn btn-filled btn-sm">{icon("edit")}继续这一轮</button><button class="btn btn-outlined btn-sm">{icon("download")}导出摘要</button></div>
        </div>
      </div>
    </main>
  </div>''')

SCREENS += [review_start, review_complete, history]

# ---------------- 16 batch rename ----------------
batch_rename = screen("batch-rename", "批量重命名", workspace_bg() + dialog(
    "批量重命名",
    f'''
    <p style="margin:0 0 14px">对当前筛选中的 12 个素材重命名。原文件名会写入评审历史，可恢复。</p>
    <div class="field"><label>命名模板</label><div class="value mono">{{name}}-{{index:02}}</div></div>
    <div class="row gap-8 mt-8"><span class="chip selected">保留扩展名</span><span class="chip">序号从 01 开始</span><span class="chip">冲突时跳过</span></div>
    <div class="batch-preview">
      <div class="batch-row"><span class="old">登录页-桌面.png</span><span class="arrow">→</span><span class="new">登录页-桌面-01.png</span></div>
      <div class="batch-row"><span class="old">数据看板.png</span><span class="arrow">→</span><span class="new">数据看板-02.png</span></div>
      <div class="batch-row"><span class="old">设置页-深色.png</span><span class="arrow">→</span><span class="new">设置页-深色-03.png</span></div>
      <div class="batch-row"><span class="old">移动端-首页.png</span><span class="arrow">→</span><span class="new">移动端-首页-04.png</span></div>
    </div>
    <p class="small muted mt-8" style="margin-bottom:0">还有 8 个文件未显示。重命名是原子操作，失败时不会留下部分结果。</p>
    ''',
    '<button class="btn btn-text">取消</button><button class="btn btn-filled">重命名 12 个文件</button>',
    cls="lg", icon_name="edit"))

# ---------------- 17 batch image ----------------
batch_image = screen("batch-image", "批量处理图片", workspace_bg() + dialog(
    "批量处理图片",
    f'''
    <p style="margin:0 0 14px">对当前筛选中的 24 张图片生成副本，不覆盖原文件。</p>
    <div class="row gap-12">
      <div class="field grow"><label>最长边</label><div class="value">2400 px</div></div>
      <div class="field grow"><label>输出格式</label><div class="value">WebP</div></div>
      <div class="field grow"><label>质量</label><div class="value">82</div></div>
    </div>
    <div class="mt-12"><div class="progress-linear"><span style="width:100%"></span></div></div>
    <div class="row gap-8 mt-12" style="font-size:12px"><span class="status-dot"></span><span>预计输出 24 个文件 · 约 18 MB</span><span style="margin-left:auto" class="muted">原文件保持不变</span></div>
    <div class="settings-section" style="margin-top:14px">
      <div class="settings-row" style="padding:12px 14px"><div class="grow"><div class="name">输出目录</div><div class="desc">项目内的 <span class="mono">.viewer/output/2026-10-28</span></div></div><button class="btn btn-outlined btn-sm">更改</button></div>
    </div>
    ''',
    '<button class="btn btn-text">取消</button><button class="btn btn-filled">开始处理</button>',
    cls="", icon_name="image"))

# ---------------- 18 settings ----------------
settings = screen("settings", "设置 · 语音", f'''
  {topbar('设置', f'<div class="avatar">D</div>', search=False)}
  <div class="settings-layout">
    <aside class="settings-nav">
      <div class="tree-section" style="margin-top:4px">应用</div>
      <div class="tree-row">通用</div>
      <div class="tree-row">外观</div>
      <div class="tree-row active">语音输入</div>
      <div class="tree-row">快捷键</div>
      <div class="tree-row">存储</div>
      <div class="tree-section">关于</div>
      <div class="tree-row">版本与许可证</div>
    </aside>
    <main class="settings-main">
      <div class="settings-title">语音输入</div>
      <div class="settings-sub">把口述内容转成评审文字。音频与识别都在本机完成。</div>
      <div class="settings-section">
        <div class="settings-row">
          <div class="model-badge">{icon("mic")}</div>
          <div class="grow"><div class="name">Qwen3-ASR 0.6B</div><div class="desc">本地模型 · 约 937 MB · 中文 / 方言 / English · int8</div></div>
          <span class="chip status-ok">已就绪</span>
          <button class="btn btn-outlined btn-sm">更改</button>
        </div>
        <div class="settings-row">
          <div class="grow"><div class="name">识别语言</div><div class="desc">自动检测以中文为主；也可以固定语言提升短句准确率。</div></div>
          <div class="segmented"><button class="selected">自动</button><button>中文</button><button>English</button></div>
        </div>
        <div class="settings-row">
          <div class="grow"><div class="name">麦克风权限</div><div class="desc">首次录音时由 macOS 请求。Viewer 只在你按下录音后采集。</div></div>
          <span class="chip status-ok">已允许</span>
          <button class="btn btn-text btn-sm">系统设置</button>
        </div>
      </div>
      <div class="side-title" style="margin-top:24px">隐私</div>
      <div class="settings-section" style="margin-top:8px">
        <div class="settings-row"><div class="grow"><div class="name">音频不写入磁盘</div><div class="desc">录音只在内存中停留，转写结束、失败或取消后立即丢弃。日志不记录识别文本。</div></div>{icon("check_circle", "icon")}</div>
        <div class="settings-row"><div class="grow"><div class="name">完全离线</div><div class="desc">模型随应用分发，不调用云端 API；断网也能使用。</div></div>{icon("check_circle", "icon")}</div>
      </div>
      <div class="row gap-8 mt-16"><button class="btn btn-filled btn-sm">{icon("mic")}测试麦克风</button><button class="btn btn-text btn-sm">查看模型许可证</button></div>
      <p class="small muted mt-12" style="line-height:18px">模型准确率与延迟以 0.3.0 的 S0 实测为准；若中文 CER 不达标，回退 Whisper turbo / small。</p>
    </main>
  </div>''')

# ---------------- 19 conflict ----------------
conflict = screen("conflict", "冲突对话框", workspace_bg() + dialog(
    "素材已在外部修改",
    f'''
    <p style="margin:0 0 12px"><strong>数据看板.png</strong> 在 Viewer 外被修改。当前评审基于版本 v4，磁盘上是 v5。</p>
    <div class="settings-section" style="margin-top:0">
      <div class="settings-row" style="padding:12px 14px"><div class="grow"><div class="name">当前评审</div><div class="desc">v4 · 42 条意见 · 今天 17:24</div></div><span class="badge">基于旧版</span></div>
      <div class="settings-row" style="padding:12px 14px"><div class="grow"><div class="name">磁盘文件</div><div class="desc">v5 · 2880×1800 · 今天 18:02</div></div><span class="badge" style="background:#ffefc6;color:#5b4300">新版本</span></div>
    </div>
    <p class="small muted mt-12" style="margin-bottom:0">重新加载新版本会保留意见文字，但旧坐标可能偏移。建议先查看差异，或保留当前评审并另存副本。</p>
    ''',
    '<button class="btn btn-text">取消</button><button class="btn btn-outlined">保留当前评审</button><button class="btn btn-filled">重新加载 v5</button>',
    icon_name="warning"))

# ---------------- 20 empty search ----------------
empty_search = screen("empty-search", "搜索无结果", topbar(
    '<span>测试图</span><span>/</span><strong>搜索</strong>',
    f'<button class="icon-btn" aria-label="通知">{icon("notifications")}</button><div class="avatar">D</div>').replace(
        '<span>搜索素材、文件夹或意见</span>', '<span style="color:var(--md-on-surface);font-weight:500">登录</span><span style="margin-left:auto">⌘K</span>').replace(
        '<span class="kbd">⌘K</span>', '<button class="icon-btn icon-btn-sm" aria-label="清除搜索">' + icon("close", "icon-sm") + '</button>') + f'''
  <div class="app-body">
    {rail()}
    {pane()}
    <main class="main">
      <div class="content-head">
        <div><div class="content-title">搜索结果</div><div class="content-sub">“登录” · 0 个素材 · 0 条意见</div></div>
        <div class="content-actions"><button class="btn btn-outlined btn-sm">{icon("filter_list")}筛选</button></div>
      </div>
      <div class="content-toolbar"><span class="chip selected">全部</span><span class="chip">素材</span><span class="chip">文件夹</span><span class="chip">评审意见</span></div>
      <div class="center grow" style="flex-direction:column;gap:8px;text-align:center;padding-bottom:80px">
        <div style="width:72px;height:72px;border-radius:20px;background:var(--md-surface-container-high);display:flex;align-items:center;justify-content:center;color:var(--md-on-surface-variant)">{icon("search", "icon-xl")}</div>
        <div style="font-size:18px;font-weight:700">没有找到“登录”</div>
        <div class="small muted" style="max-width:360px;line-height:18px">试试更短的关键词，或检查是否开启了筛选条件。搜索会匹配文件名、文件夹和意见文字。</div>
        <div class="row gap-8 mt-8"><button class="btn btn-filled btn-sm">清除搜索</button><button class="btn btn-outlined btn-sm">清除筛选</button></div>
      </div>
    </main>
  </div>''')

# ---------------- 21 migration ----------------
migration = screen("migration", "旧版评审迁移", workspace_bg() + dialog(
    "迁移旧版评审",
    f'''
    <p style="margin:0 0 12px">这个项目使用旧版评审格式，只支持矩形标注。迁移后可以继续使用全部标记工具、样式与语音输入。</p>
    <div class="settings-section" style="margin-top:0">
      <div class="settings-row" style="padding:12px 14px"><div class="grow"><div class="name">当前格式</div><div class="desc">旧版评审 · 只读 · 18 条意见</div></div><span class="badge" style="background:#ffefc6;color:#5b4300">需要迁移</span></div>
      <div class="settings-row" style="padding:12px 14px"><div class="grow"><div class="name">迁移后</div><div class="desc">持续评审 v5 · 保留原有矩形与文字 · 可撤销</div></div>{icon("check_circle", "icon")}</div>
    </div>
    <p class="small muted mt-12" style="margin-bottom:0">迁移会写入新的评审记录，不会修改原素材。旧版 Viewer 将无法再打开迁移后的评审。</p>
    ''',
    '<button class="btn btn-text">稍后</button><button class="btn btn-filled">迁移并继续</button>',
    icon_name="history"))

SCREENS += [batch_rename, batch_image, settings, conflict, empty_search, migration]
SCREENS.append(tool_sheet())

# ---------------- output ----------------
css = (ROOT / "assets" / "design.css").read_text()
html = f'''<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="icon" href="data:,">
<title>Viewer 0.3.0 · 视觉规格</title>
<style>
{css}
body.capture {{ background: #fff; }}
body.capture .screen {{ margin: 0; }}
.tool .caret {{
  position: absolute; right: 4px; bottom: 3px; font-size: 7px; line-height: 1;
  color: currentColor; opacity: .45;
}}
.check-box {{
  width: 18px; height: 18px; border-radius: 4px; border: 1px solid var(--md-outline);
  display: inline-flex; align-items: center; justify-content: center; color: transparent;
}}
.check-box.checked {{ background: var(--md-primary); border-color: var(--md-primary); color: #fff; }}
.check-box .icon {{ width: 14px; height: 14px; }}
</style>
</head>
<body>
{SPRITE}
{''.join(SCREENS)}
<script>
const params = new URLSearchParams(location.search);
const id = params.get('screen');
const screens = [...document.querySelectorAll('.screen')];
if (id) {{
  document.body.classList.add('capture');
  screens.forEach(s => s.hidden = s.id !== id);
  document.title = (screens.find(s => s.id === id)?.dataset.title || id) + ' · Viewer 0.3.0';
}} else {{
  screens.forEach(s => s.hidden = false);
}}
</script>
</body>
</html>'''
(ROOT / "screens.html").write_text(html)
print(f"wrote {ROOT/'screens.html'} ({len(html)} bytes, {len(SCREENS)} screens)")

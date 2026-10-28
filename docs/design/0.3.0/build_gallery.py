"""Build the static contact sheet from screens.html and screens/*.png."""
import re
from pathlib import Path

html = Path('screens.html').read_text()
items = re.findall(r'<section class="screen[^"]*" id="([^"]+)" data-title="([^"]+)"', html)
cards = "\n".join(
    f'''<a class="card" href="screens.html?screen={sid}" target="_blank">
      <img src="screens/{sid}.png" alt="{title}">
      <div class="cap"><span class="num">{i:02d}</span><span class="name">{title}</span><span class="file">{sid}.png</span></div>
    </a>''' for i, (sid, title) in enumerate(items, 1)
)
page = f'''<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="icon" href="data:,">
<title>Viewer 0.3.0 · 视觉稿总览</title>
<style>
  @font-face {{ font-family: Roboto; src: url(assets/Roboto.woff2) format("woff2"); font-weight: 100 900; }}
  * {{ box-sizing: border-box; }}
  body {{ margin:0; background:#eef3f1; color:#171d1c; font-family:Roboto,"PingFang SC",sans-serif; }}
  header {{ padding:32px 40px 20px; max-width:1500px; margin:0 auto; display:flex; align-items:flex-end; gap:20px; }}
  h1 {{ font-size:28px; margin:0; letter-spacing:-.5px; }}
  p {{ margin:4px 0 0; color:#3f4948; font-size:13px; }}
  .meta {{ margin-left:auto; font-size:12px; color:#3f4948; text-align:right; }}
  main {{ max-width:1500px; margin:0 auto; padding:0 40px 48px; display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:22px; }}
  .card {{ background:#fff; border:1px solid #bec9c7; border-radius:14px; overflow:hidden; text-decoration:none; color:inherit; transition:border-color .15s, box-shadow .15s; }}
  .card:hover {{ border-color:#00696d; box-shadow:0 4px 14px rgba(0,0,0,.08); }}
  img {{ width:100%; aspect-ratio:16/10; object-fit:cover; display:block; background:#d6dbd9; }}
  .cap {{ padding:10px 12px; display:flex; align-items:center; gap:8px; font-size:12px; }}
  .num {{ font-family:monospace; color:#00696d; font-weight:700; }}
  .name {{ font-weight:600; }}
  .file {{ margin-left:auto; color:#6f7978; font-size:10px; font-family:monospace; }}
  @media (max-width:1100px) {{ main {{ grid-template-columns:repeat(2,minmax(0,1fr)); padding:0 24px 32px; }} header {{ padding:24px; }} }}
</style></head>
<body>
<header><div><h1>Viewer 0.3.0 · 视觉稿总览</h1><p>Material Design 3 · 浅色 · 1440×900 · 23 屏</p></div><div class="meta">点击任意画面查看单屏<br>源文件 screens.html</div></header>
<main>{cards}</main>
</body></html>'''
Path('gallery.html').write_text(page)
print(f'gallery: {len(items)} screens')

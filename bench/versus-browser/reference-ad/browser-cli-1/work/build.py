#!/usr/bin/env python3
"""Build the vote-by-mail flyer at three sizes from one master design."""
import subprocess, os, pathlib

HERE = pathlib.Path(__file__).parent.resolve()
PW = "/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/.bin/playwright"

NAVY = "#1B2A5C"
RED = "#D0202E"

SIZES = [
    ("portrait", 1080, 1350, 1.00),
    ("wide",     1200, 1000, 0.85),
    ("sky",       300,  600, 0.28),
]

CANDIDATES = [("Dana Levi", "Mayor"), ("Omar Haddad", "Council"), ("Ruth Cohen", "Council")]
STEPS = ["Request your ballot by October 20", "Fill it out at home", "Mail it back by November 3"]


def html(name, W, H, s):
    def p(v):            # scaled px value
        return f"{round(v * s, 3)}px"

    photo_max = round(W * 0.62, 2)
    photo_min = round(W * 0.22, 2)

    cards = "\n".join(
        f'''      <div class="card">
        <span class="bar"></span>
        <div class="cname">{n}</div>
        <div class="crole">{r}</div>
      </div>''' for n, r in CANDIDATES)

    steps = "\n".join(
        f'''      <li><img class="chk" src="check.svg" alt=""><span>{t}</span></li>'''
        for t in STEPS)

    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<style>
  @font-face {{
    font-family: "Inter";
    src: url("Inter.ttf") format("truetype");
    font-weight: 100 900;
    font-style: normal;
  }}
  * {{ margin:0; padding:0; box-sizing:border-box; }}
  html, body {{ width:{W}px; height:{H}px; overflow:hidden; }}
  body {{
    font-family:"Inter", sans-serif;
    background:#fff;
    color:{NAVY};
    -webkit-font-smoothing:antialiased;
    display:flex; flex-direction:column;
    justify-content:space-evenly;
    padding:{p(52)} 0 {p(44)};
    row-gap:{p(34)};
  }}
  .pad {{ padding-left:{p(60)}; padding-right:{p(60)}; }}

  /* 1. headline */
  h1 {{
    font-size:{p(70)}; line-height:1.08; font-weight:800;
    letter-spacing:{p(-1.6)}; text-wrap:balance;
  }}
  h1 em {{ color:{RED}; font-style:normal; }}

  /* 2. photo band */
  .photo {{
    width:100%; flex:1 1 auto;
    min-height:{photo_min}px; max-height:{photo_max}px;
    overflow:hidden; display:block;
  }}
  .photo img {{ width:100%; height:100%; object-fit:cover; object-position:50% 70%; display:block; }}

  /* 3. candidates */
  .cands {{ display:grid; grid-template-columns:repeat(3, 1fr); gap:{p(22)}; }}
  .card {{
    background:#F1F4FB; border-radius:{p(14)};
    padding:{p(20)} {p(14)} {p(22)};
    text-align:center; display:flex; flex-direction:column; align-items:center;
  }}
  .bar {{ display:block; width:{p(40)}; height:{p(5)}; background:{RED}; border-radius:{p(3)}; }}
  .cname {{ font-size:{p(34)}; font-weight:700; line-height:1.15; margin-top:{p(14)}; white-space:nowrap; }}
  .crole {{
    font-size:{p(20)}; font-weight:600; color:{RED};
    text-transform:uppercase; letter-spacing:{p(1.2)}; margin-top:{p(6)};
  }}

  /* 4. CTA bar */
  .cta {{
    width:100%; background:{RED}; color:#fff;
    display:flex; align-items:center; justify-content:center; gap:{p(22)};
    padding:{p(24)} {p(20)};
  }}
  .cta img {{ width:{p(56)}; height:{p(44)}; display:block; }}
  .cta span {{ font-size:{p(42)}; font-weight:800; letter-spacing:{p(3)}; line-height:1; white-space:nowrap; }}

  /* 5. steps */
  ol {{ list-style:none; display:flex; flex-direction:column; gap:{p(18)}; }}
  ol li {{ display:flex; align-items:center; gap:{p(16)}; }}
  .chk {{ width:{p(38)}; height:{p(38)}; flex:none; display:block; }}
  ol li span {{ font-size:{p(28)}; font-weight:500; line-height:1.25; }}

  /* 6. footer */
  footer {{ font-size:{p(21)}; font-weight:500; color:#616A8C; }}
</style></head>
<body>
  <h1 class="pad">Proven <em>RESULTS</em> for <em>WILLOWMERE</em> Families</h1>

  <figure class="photo"><img src="photo.png" alt=""></figure>

  <section class="cands pad">
{cards}
  </section>

  <div class="cta"><img src="mail.svg" alt=""><span>VOTE BY MAIL</span></div>

  <ol class="pad">
{steps}
  </ol>

  <footer class="pad">Paid for by Willowmere Forward &middot; willowmereforward.org</footer>
</body></html>
'''


for name, W, H, s in SIZES:
    f = HERE / f"{name}.html"
    f.write_text(html(name, W, H, s))
    out = HERE / f"{name}.png"
    subprocess.run([PW, "screenshot", "--viewport-size", f"{W},{H}",
                    f"file://{f}", str(out)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(name, subprocess.run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(out)],
                               capture_output=True, text=True).stdout.split("\n")[1:3])

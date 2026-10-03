const fs = require('fs');

const CFG = {
  square: {
    w: 1080, h: 1080, pad: 76, layout: 'row',
    logoW: 168, eventFs: 21,
    ring: 232, nameFs: 42, roleFs: 23,
    titleFs: 68, titleLh: 1.13,
    metaFs: 24, btnFs: 23, btnPadV: 20, btnPadH: 42,
    groupGap: 34,
  },
  landscape: {
    w: 1920, h: 1080, pad: 88, layout: 'split',
    logoW: 186, eventFs: 24,
    ring: 430, nameFs: 48, roleFs: 27,
    titleFs: 80, titleLh: 1.12,
    metaFs: 27, btnFs: 26, btnPadV: 23, btnPadH: 50,
    groupGap: 34,
  },
  story: {
    w: 1080, h: 1920, pad: 84, layout: 'stack',
    logoW: 178, eventFs: 23,
    ring: 466, nameFs: 62, roleFs: 31,
    titleFs: 102, titleLh: 1.11,
    metaFs: 30, btnFs: 29, btnPadV: 26, btnPadH: 56,
    groupGap: 46,
  },
};

const logo = fs.readFileSync('logo.svg', 'utf8').trim();

const head = `<header class="head">
      <span class="logo">${logo}</span>
      <span class="bar"></span>
      <span class="event">FIELDNOTES CONF 2026</span>
    </header>`;

const portrait = `<div class="ring"><div class="photo"></div></div>`;
const who = `<div class="who">
          <h2 class="name">Maya Okonkwo-Lindqvist</h2>
          <p class="role">Principal Engineer, Northwind Labs</p>
        </div>`;
const title = `<h1 class="title">Shipping at the speed of trust: what ten years of on‑call taught us about resilient systems</h1>`;
const hr = `<div class="hr"></div>`;
const meta = `<p class="meta"><span class="dot"></span>November 14, 2026 <span class="sep">·</span> Harbor Hall, Lisbon</p>`;
const btn = `<a class="btn">Get tickets</a>`;

function body(c) {
  if (c.layout === 'split') {
    return `${head}
    <main class="main split">
      <div class="col-l">${portrait}</div>
      <div class="col-r">
        ${who}
        <div class="group">
          ${hr}
          ${title}
          <div class="foot">${meta}${btn}</div>
        </div>
      </div>
    </main>`;
  }
  const speaker = c.layout === 'row'
    ? `<div class="speaker">${portrait}${who}</div>`
    : `<div class="speaker col">${portrait}${who}</div>`;
  return `${head}
    <main class="main stackmain">
      ${speaker}
      <div class="group">
        ${hr}
        ${title}
        <div class="foot">${meta}${btn}</div>
      </div>
    </main>`;
}

function css(c) {
  const split = c.layout === 'split';
  return `
@font-face{font-family:'Inter';src:url('Inter.ttf') format('truetype');font-weight:100 900;font-style:normal;}
*{margin:0;padding:0;box-sizing:border-box;}
html,body{width:${c.w}px;height:${c.h}px;}
body{font-family:'Inter',sans-serif;background:#0F1226;color:#fff;-webkit-font-smoothing:antialiased;overflow:hidden;}

.card{position:relative;width:${c.w}px;height:${c.h}px;padding:${c.pad}px;
  display:flex;flex-direction:column;overflow:hidden;}
.card::before{content:'';position:absolute;inset:0;pointer-events:none;
  background:
    radial-gradient(${split ? '1000px 820px at 16% 6%' : '860px 720px at 12% 2%'},rgba(124,92,255,.32),transparent 62%),
    radial-gradient(820px 700px at 104% 100%,rgba(124,92,255,.16),transparent 60%);}
.card::after{content:'';position:absolute;left:0;right:0;top:0;height:6px;
  background:linear-gradient(90deg,#7C5CFF 0%,#7C5CFF 40%,rgba(124,92,255,0) 100%);}
.head,.main{position:relative;z-index:1;}

/* header */
.head{display:flex;align-items:center;gap:${Math.round(c.eventFs * 1.05)}px;flex:0 0 auto;}
.logo{display:block;line-height:0;}
.logo svg{display:block;width:${c.logoW}px;height:auto;}
.bar{width:2px;height:${Math.round(c.eventFs * 1.9)}px;background:rgba(255,255,255,.26);border-radius:2px;flex:0 0 auto;}
.event{font-size:${c.eventFs}px;font-weight:700;letter-spacing:0.20em;
  color:rgba(255,255,255,.94);white-space:nowrap;line-height:1;padding-left:2px;}

/* main */
.main{flex:1 1 auto;display:flex;min-height:0;min-width:0;}
.stackmain{flex-direction:column;justify-content:space-between;
  padding-top:${Math.round(c.pad * 0.5)}px;padding-bottom:${Math.round(c.pad * 0.1)}px;}
.split{flex-direction:row;align-items:center;gap:${Math.round(c.pad * 0.95)}px;}
.col-l{flex:0 0 ${c.ring}px;display:flex;}
.col-r{flex:1 1 auto;min-width:0;display:flex;flex-direction:column;justify-content:center;
  gap:${Math.round(c.titleFs * 0.5)}px;}

/* portrait */
.ring{flex:0 0 auto;width:${c.ring}px;height:${c.ring}px;border-radius:50%;
  padding:${Math.max(4, Math.round(c.ring * 0.026))}px;
  background:linear-gradient(145deg,#7C5CFF,rgba(124,92,255,.16));
  box-shadow:0 ${Math.round(c.ring * 0.055)}px ${Math.round(c.ring * 0.16)}px rgba(6,8,24,.55);}
.photo{width:100%;height:100%;border-radius:50%;
  background:url('portrait.png') center/cover no-repeat,#1b1f3a;}

/* speaker */
.speaker{display:flex;align-items:center;gap:${Math.round(c.ring * 0.16)}px;flex:0 0 auto;min-width:0;}
.speaker.col{flex-direction:column;align-items:flex-start;gap:${Math.round(c.ring * 0.11)}px;}
.who{min-width:0;}
.name{font-size:${c.nameFs}px;font-weight:700;letter-spacing:-0.022em;line-height:1.1;
  overflow-wrap:break-word;}
.role{margin-top:${Math.round(c.roleFs * 0.46)}px;font-size:${c.roleFs}px;font-weight:500;
  line-height:1.35;color:#B9A7FF;overflow-wrap:break-word;}

/* title + footer group */
.group{display:flex;flex-direction:column;gap:${Math.round(c.titleFs * 0.62)}px;flex:0 0 auto;min-width:0;}
.hr{height:2px;width:100%;flex:0 0 auto;border-radius:2px;
  background:linear-gradient(90deg,rgba(124,92,255,.85) 0%,rgba(124,92,255,.35) 32%,rgba(255,255,255,.07) 100%);}
.title{font-size:${c.titleFs}px;font-weight:700;line-height:${c.titleLh};letter-spacing:-0.026em;
  overflow-wrap:break-word;text-wrap:pretty;}
.foot{display:flex;flex-direction:column;align-items:flex-start;gap:${c.groupGap}px;flex:0 0 auto;}
.meta{display:flex;align-items:center;font-size:${c.metaFs}px;font-weight:500;line-height:1.4;
  color:rgba(255,255,255,.82);overflow-wrap:break-word;}
.dot{width:${Math.round(c.metaFs * 0.4)}px;height:${Math.round(c.metaFs * 0.4)}px;border-radius:50%;
  background:#7C5CFF;margin-right:${Math.round(c.metaFs * 0.55)}px;flex:0 0 auto;}
.sep{color:#7C5CFF;padding:0 ${Math.round(c.metaFs * 0.34)}px;font-weight:700;}
.btn{display:inline-flex;align-items:center;justify-content:center;
  padding:${c.btnPadV}px ${c.btnPadH}px;border-radius:999px;background:#7C5CFF;color:#fff;
  font-size:${c.btnFs}px;font-weight:700;letter-spacing:-0.004em;line-height:1;white-space:nowrap;
  box-shadow:0 ${Math.round(c.btnFs * 0.5)}px ${Math.round(c.btnFs * 1.3)}px rgba(124,92,255,.36);}
`;
}

for (const [k, c] of Object.entries(CFG)) {
  const html = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>${k}</title>
<style>${css(c)}</style>
</head>
<body><div class="card">
    ${body(c)}
  </div></body></html>`;
  fs.writeFileSync(`${k}.html`, html);
}
console.log('built 3 html files');

const fs = require('fs');

const EVENT = 'FIELDNOTES CONF 2026';
const NAME = '<span class="nb">Maya Okonkwo-Lindqvist</span>';
const ROLE = 'Principal Engineer, Northwind Labs';
const TITLE = 'Shipping at the speed of trust: what ten years of <span class="nb">on-call</span> taught us about resilient systems';
const META = 'November 14, 2026 · Harbor Hall, Lisbon';
const CTA = 'Get tickets';

const base = (W, H, css, body) => `<!doctype html>
<html><head><meta charset="utf-8">
<style>
@font-face{font-family:'Inter';src:url('Inter.ttf') format('truetype');font-weight:100 900;font-style:normal;font-display:block;}
*{margin:0;padding:0;box-sizing:border-box;}
html,body{width:${W}px;height:${H}px;overflow:hidden;background:#0F1226;}
body{font-family:'Inter',sans-serif;color:#fff;-webkit-font-smoothing:antialiased;font-feature-settings:"kern"1,"liga"1;}
.card{position:relative;width:${W}px;height:${H}px;background:#0F1226;overflow:hidden;}
.glow{position:absolute;inset:0;pointer-events:none;
 background:
  radial-gradient(60% 55% at 84% 8%, rgba(124,92,255,.30) 0%, rgba(124,92,255,0) 62%),
  radial-gradient(55% 50% at 6% 96%, rgba(124,92,255,.20) 0%, rgba(124,92,255,0) 60%);}
.hair{position:absolute;left:0;right:0;top:0;height:6px;
 background:linear-gradient(90deg,#7C5CFF 0%,#7C5CFF 42%,rgba(124,92,255,.15) 100%);}
.inner{position:relative;height:100%;display:flex;}
.brand{display:flex;align-items:center;gap:var(--bgap);}
.brand img{display:block;height:var(--logoh);width:auto;}
.brand .rule{width:1px;height:calc(var(--logoh)*.72);background:rgba(255,255,255,.26);}
.brand .ev{font-weight:700;font-size:var(--evs);letter-spacing:.16em;color:#EDEBFF;white-space:nowrap;}
.who{display:flex;align-items:center;gap:var(--wgap);}
.pf{position:relative;flex:0 0 auto;width:var(--pf);height:var(--pf);border-radius:50%;overflow:hidden;
 background:#1a1f3d;box-shadow:0 0 0 var(--ring) rgba(124,92,255,.85), 0 0 0 calc(var(--ring) + 2px) rgba(255,255,255,.10), 0 26px 60px rgba(0,0,0,.45);}
.pf img{width:100%;height:100%;object-fit:cover;object-position:50% 38%;display:block;}
.nm{font-weight:700;font-size:var(--nms);line-height:1.1;letter-spacing:-.02em;}
.rl{font-weight:500;font-size:var(--rls);line-height:1.35;color:#A7ADD4;margin-top:var(--rlm);}
.kicker{display:inline-flex;align-items:center;gap:.5em;font-weight:600;font-size:var(--kks);
 letter-spacing:.18em;color:#9E86FF;text-transform:uppercase;}
.kicker::before{content:"";width:var(--kkl);height:2px;background:#7C5CFF;border-radius:2px;}
.title{font-weight:700;font-size:var(--tts);line-height:1.1;letter-spacing:-.025em;
 text-wrap:pretty;overflow-wrap:break-word;hyphens:none;}
.title .hl{color:#B9A6FF;}
.nb{white-space:nowrap;}
.meta{display:flex;align-items:center;gap:var(--mgap);font-weight:500;font-size:var(--mts);color:#C7CBE6;}
.meta .dot{width:var(--mdot);height:var(--mdot);border-radius:50%;background:#7C5CFF;flex:0 0 auto;}
.cta{display:inline-flex;align-items:center;justify-content:center;gap:.55em;
 background:#7C5CFF;color:#fff;font-weight:700;font-size:var(--cts);letter-spacing:.005em;
 padding:var(--ctpy) var(--ctpx);border-radius:999px;white-space:nowrap;
 box-shadow:0 14px 34px rgba(124,92,255,.42);}
.cta svg{width:.92em;height:.92em;display:block;}
${css}
</style></head>
<body>${body}
<script>
(function(){
  var t=document.querySelector('.title');
  function over(){
    var cols=document.querySelectorAll('.col');
    for(var i=0;i<cols.length;i++){
      if(cols[i].scrollHeight>cols[i].clientHeight+0.5) return true;
      if(cols[i].scrollWidth>cols[i].clientWidth+0.5) return true;
    }
    return document.documentElement.scrollHeight>${H}||document.documentElement.scrollWidth>${W};
  }
  var s=parseFloat(getComputedStyle(t).fontSize);
  var guard=0;
  while(over()&&s>16&&guard++<400){ s-=1; t.style.fontSize=s+'px'; }
  document.documentElement.setAttribute('data-title-size',s);
  document.documentElement.setAttribute('data-overflow',over());
})();
</script>
</body></html>`;

const logo = `<div class="brand"><img src="logo.svg" alt=""><div class="rule"></div><div class="ev">${EVENT}</div></div>`;
const pf = `<div class="pf"><img src="portrait.png" alt=""></div>`;
const title = `<h1 class="title">${TITLE}</h1>`;
const meta = `<div class="meta"><span class="dot"></span><span>${META}</span></div>`;
const cta = `<div class="cta">${CTA}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h13M12 5l7 7-7 7"/></svg></div>`;

/* ---------------- SQUARE 1080x1080 ---------------- */
const square = base(1080, 1080, `
.inner{padding:76px 76px 72px;}
.col{flex:1 1 auto;width:100%;height:100%;display:flex;flex-direction:column;overflow:hidden;
 --logoh:44px;--bgap:20px;--evs:19px;
 --pf:172px;--wgap:30px;--ring:4px;--nms:39px;--rls:21px;--rlm:8px;
 --kks:16px;--kkl:34px;--tts:68px;
 --mgap:14px;--mts:22px;--mdot:9px;
 --cts:23px;--ctpy:20px;--ctpx:40px;}
.hdr{flex:0 0 auto;}
.mid{flex:1 1 auto;display:flex;flex-direction:column;justify-content:center;gap:44px;padding:40px 0;min-height:0;}
.ftr{flex:0 0 auto;display:flex;align-items:center;justify-content:space-between;gap:28px;
 border-top:1px solid rgba(255,255,255,.12);padding-top:34px;}
`, `<div class="card"><div class="glow"></div><div class="hair"></div><div class="inner"><div class="col">
  <div class="hdr">${logo}</div>
  <div class="mid">
    <div class="who">${pf}<div><div class="nm">${NAME}</div><div class="rl">${ROLE}</div></div></div>
    ${title}
  </div>
  <div class="ftr">${meta}${cta}</div>
</div></div></div>`);

/* ---------------- LANDSCAPE 1920x1080 ---------------- */
const landscape = base(1920, 1080, `
.inner{padding:88px 96px 84px;gap:80px;align-items:stretch;}
.col{height:100%;display:flex;flex-direction:column;overflow:hidden;
 --logoh:46px;--bgap:22px;--evs:20px;
 --pf:420px;--wgap:0px;--ring:5px;--nms:42px;--rls:23px;--rlm:10px;
 --kks:17px;--kkl:36px;--tts:74px;
 --mgap:15px;--mts:24px;--mdot:10px;
 --cts:25px;--ctpy:22px;--ctpx:46px;}
.left{flex:1 1 auto;min-width:0;}
.right{flex:0 0 600px;align-items:center;justify-content:center;text-align:center;}
.hdr{flex:0 0 auto;}
.mid{flex:1 1 auto;display:flex;flex-direction:column;justify-content:center;gap:30px;padding:40px 0;min-height:0;}
.ftr{flex:0 0 auto;display:flex;align-items:center;gap:44px;
 border-top:1px solid rgba(255,255,255,.12);padding-top:36px;}
.right .who{flex-direction:column;gap:36px;}
.right .nm{font-size:var(--nms);white-space:nowrap;}
.right .rl{max-width:600px;}
`, `<div class="card"><div class="glow"></div><div class="hair"></div><div class="inner">
  <div class="col left">
    <div class="hdr">${logo}</div>
    <div class="mid"><div class="kicker">Keynote</div>${title}</div>
    <div class="ftr">${meta}${cta}</div>
  </div>
  <div class="col right">
    <div class="who">${pf}<div><div class="nm">${NAME}</div><div class="rl">${ROLE}</div></div></div>
  </div>
</div></div>`);

/* ---------------- STORY 1080x1920 ---------------- */
const story = base(1080, 1920, `
.inner{padding:110px 84px 120px;}
.col{flex:1 1 auto;width:100%;height:100%;display:flex;flex-direction:column;overflow:hidden;
 --logoh:48px;--bgap:22px;--evs:21px;
 --pf:348px;--wgap:0px;--ring:5px;--nms:50px;--rls:26px;--rlm:12px;
 --kks:19px;--kkl:38px;--tts:80px;
 --mgap:16px;--mts:27px;--mdot:11px;
 --cts:29px;--ctpy:27px;--ctpx:58px;}
.hdr{flex:0 0 auto;display:flex;justify-content:center;}
.mid{flex:1 1 auto;display:flex;flex-direction:column;align-items:center;justify-content:center;
 gap:62px;padding:64px 0;min-height:0;text-align:center;}
.mid .who{flex-direction:column;gap:32px;}
.mid .title{margin-top:4px;text-wrap:balance;}
.ftr{flex:0 0 auto;display:flex;flex-direction:column;align-items:center;gap:40px;
 border-top:1px solid rgba(255,255,255,.12);padding-top:52px;}
.kicker{align-self:center;}
`, `<div class="card"><div class="glow"></div><div class="hair"></div><div class="inner"><div class="col">
  <div class="hdr">${logo}</div>
  <div class="mid">
    <div class="who">${pf}<div><div class="nm">${NAME}</div><div class="rl">${ROLE}</div></div></div>
    <div class="kicker">Keynote</div>
    ${title}
  </div>
  <div class="ftr">${meta}${cta}</div>
</div></div></div>`);

fs.writeFileSync('square.html', square);
fs.writeFileSync('landscape.html', landscape);
fs.writeFileSync('story.html', story);
console.log('written');

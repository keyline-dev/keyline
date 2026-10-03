const fs = require('fs');
const path = require('path');
const dir = __dirname;

const tpl = (s) => `<!DOCTYPE html>
<html><head><meta charset="utf-8">
<style>
@font-face{font-family:'Inter';src:url('Inter.ttf') format('truetype');font-weight:100 900;font-style:normal;}
*{margin:0;padding:0;box-sizing:border-box;}
html{font-size:${(s * 10).toFixed(4)}px;}
body{
  font-family:'Inter',sans-serif;
  width:100vw;height:100vh;overflow:hidden;background:#fff;
  color:#1B2A5C;
  display:flex;flex-direction:column;align-items:stretch;justify-content:space-between;
  -webkit-font-smoothing:antialiased;
}
.sp{flex-grow:1;flex-shrink:1;}
.band{flex:0 0 auto;width:100%;}
.inner{width:100%;max-width:88rem;margin:0 auto;padding:0 4.4rem;}

/* 1. headline */
.headline{flex:0 0 auto;}
.headline h1{
  font-size:6.0rem;line-height:1.08;font-weight:800;letter-spacing:-0.018em;
  text-align:center;color:#1B2A5C;text-wrap:balance;
}
.headline .r{color:#D0202E;}

/* 2. photo band */
.photo{flex:4 1 34rem;min-height:24rem;max-height:92rem;width:100%;overflow:hidden;background:#dfe6ef;}
.photo img{display:block;width:100%;height:100%;object-fit:cover;object-position:32% 58%;}

/* 3. candidates */
.cands{display:grid;grid-template-columns:repeat(3,1fr);align-items:start;}
.cand{text-align:center;padding:0 0.6rem;}
.cand .rule{width:4.4rem;height:0.4rem;background:#D0202E;margin:0 auto 1.2rem;border-radius:0.2rem;}
.cand .nm{font-size:3.2rem;line-height:1.12;font-weight:700;color:#1B2A5C;letter-spacing:-0.012em;white-space:nowrap;}
.cand .rl{margin-top:0.5rem;font-size:1.9rem;line-height:1.2;font-weight:600;letter-spacing:0.14em;text-transform:uppercase;color:#D0202E;white-space:nowrap;}

/* 4. CTA bar */
.cta{background:#D0202E;}
.cta .inner{display:flex;align-items:center;justify-content:center;gap:2.2rem;padding-top:2.6rem;padding-bottom:2.6rem;}
.cta img{width:5.6rem;height:4.4rem;display:block;flex:0 0 auto;}
.cta span{font-size:4.2rem;line-height:1.1;font-weight:800;letter-spacing:0.07em;color:#fff;white-space:nowrap;}

/* 5. steps */
.steps .inner{display:flex;flex-direction:column;align-items:center;}
.steplist{display:flex;flex-direction:column;gap:1.9rem;width:fit-content;max-width:100%;}
.step{display:flex;align-items:center;gap:1.5rem;}
.step img{width:4rem;height:4rem;display:block;flex:0 0 auto;}
.step span{font-size:2.6rem;line-height:1.2;font-weight:500;color:#1B2A5C;white-space:nowrap;}

/* 6. footer */
.footer{text-align:center;}
.footer span{font-size:1.9rem;line-height:1.3;font-weight:500;color:#6A7490;letter-spacing:0.012em;white-space:nowrap;}
</style></head>
<body>
  <div class="sp" style="flex-basis:4.2rem;max-height:9rem"></div>

  <div class="headline band"><div class="inner">
    <h1>Proven <span class="r">RESULTS</span> for <span class="r">WILLOWMERE</span> Families</h1>
  </div></div>

  <div class="sp" style="flex-basis:3.6rem;max-height:7.5rem"></div>

  <div class="photo band"><img src="photo.jpg" alt=""></div>

  <div class="sp" style="flex-basis:3.8rem;max-height:7.5rem"></div>

  <div class="band"><div class="inner cands">
    <div class="cand"><div class="rule"></div><div class="nm">Dana Levi</div><div class="rl">Mayor</div></div>
    <div class="cand"><div class="rule"></div><div class="nm">Omar Haddad</div><div class="rl">Council</div></div>
    <div class="cand"><div class="rule"></div><div class="nm">Ruth Cohen</div><div class="rl">Council</div></div>
  </div></div>

  <div class="sp" style="flex-basis:3.8rem;max-height:7.5rem"></div>

  <div class="cta band"><div class="inner">
    <img src="mail.svg" alt=""><span>VOTE BY MAIL</span>
  </div></div>

  <div class="sp" style="flex-basis:3.6rem;max-height:7.5rem"></div>

  <div class="steps band"><div class="inner"><div class="steplist">
    <div class="step"><img src="check.svg" alt=""><span>Request your ballot by October 20</span></div>
    <div class="step"><img src="check.svg" alt=""><span>Fill it out at home</span></div>
    <div class="step"><img src="check.svg" alt=""><span>Mail it back by November 3</span></div>
  </div></div></div>

  <div class="sp" style="flex-basis:3.2rem;max-height:6.5rem"></div>

  <div class="footer band"><div class="inner">
    <span>Paid for by Willowmere Forward &middot; willowmereforward.org</span>
  </div></div>

  <div class="sp" style="flex-basis:4rem;max-height:9rem"></div>
</body></html>`;

const sizes = [
  { name: 'portrait', w: 1080, h: 1350, s: 1 },
  { name: 'wide', w: 1200, h: 1000, s: 0.85 },
  { name: 'sky', w: 300, h: 600, s: 0.28 },
];
for (const z of sizes) {
  fs.writeFileSync(path.join(dir, z.name + '.html'), tpl(z.s));
}
console.log('ok');

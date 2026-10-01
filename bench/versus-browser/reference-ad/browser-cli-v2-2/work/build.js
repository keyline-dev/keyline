const fs = require('fs');
const path = require('path');
const dir = __dirname;

const page = (cfg) => `<!doctype html>
<html lang="en" class="${cfg.mode}">
<head>
<meta charset="utf-8">
<title>${cfg.name}</title>
<style>
@font-face{
  font-family:"Inter";
  src:url("Inter.ttf") format("truetype");
  font-weight:100 900;
  font-style:normal;
  font-display:block;
}
:root{
  --s:${cfg.s};
  --navy:#1B2A5C;
  --red:#D0202E;
  --ink:#2A3356;
  --pad:calc(52px * var(--s));
}
*{margin:0;padding:0;box-sizing:border-box;}
html{font-size:calc(16px * var(--s));}
body{
  width:100vw;height:100vh;overflow:hidden;
  background:#FFFFFF;
  font-family:"Inter",sans-serif;
  font-synthesis:none;
  -webkit-font-smoothing:antialiased;
  text-rendering:geometricPrecision;
  display:flex;flex-direction:column;
  justify-content:space-between;
  row-gap:calc(1.6rem);
}
/* ---------- shared blocks ---------- */
.pad{padding-left:var(--pad);padding-right:var(--pad);}
.band{width:100%;flex:none;}

/* 1. headline */
.headline{
  margin-top:calc(2.9rem);
  font-size:4.15rem;
  line-height:1.045;
  font-weight:800;
  letter-spacing:-0.022em;
  color:var(--navy);
  text-wrap:balance;
  flex:none;
}
.headline em{font-style:normal;color:var(--red);}

/* 2. photo band */
.photo{
  flex:1 1 auto;
  min-height:calc(9rem);
  max-height:calc(34rem);
  width:100%;
  overflow:hidden;
  position:relative;
  background:#8DB8E3;
}
.photo img{width:100%;height:100%;object-fit:cover;object-position:50% 42%;display:block;}

/* 3. candidates */
.cands{
  display:grid;
  grid-template-columns:repeat(3,minmax(0,1fr));
  column-gap:calc(1.75rem);
  align-items:start;
  flex:none;
}
.cand{display:flex;flex-direction:column;align-items:center;text-align:center;min-width:0;}
.mono{
  width:5.6rem;height:5.6rem;border-radius:50%;
  background:var(--navy);color:#fff;
  display:flex;align-items:center;justify-content:center;
  font-size:2.1rem;font-weight:700;letter-spacing:0.01em;
  box-shadow:0 0 0 calc(0.19rem) #fff, 0 0 0 calc(0.38rem) var(--red);
  flex:none;
}
.cand .nm{
  margin-top:calc(1.05rem);
  font-size:1.82rem;font-weight:700;color:var(--navy);
  line-height:1.15;letter-spacing:-0.012em;white-space:nowrap;
}
.cand .rl{
  margin-top:calc(0.42rem);
  font-size:1.0rem;font-weight:700;color:var(--red);
  letter-spacing:0.17em;text-transform:uppercase;white-space:nowrap;
}

/* 4. CTA bar */
.cta{
  background:var(--red);
  display:flex;align-items:center;justify-content:center;
  column-gap:calc(1.3rem);
  padding:calc(1.4rem) var(--pad);
}
.cta img{height:2.6rem;width:auto;display:block;flex:none;}
.cta span{
  color:#fff;font-size:2.65rem;font-weight:800;
  letter-spacing:0.07em;line-height:1.2;white-space:nowrap;
}

/* 5. steps */
.steps{display:flex;flex-direction:column;row-gap:calc(1.05rem);flex:none;}
.step{display:flex;align-items:center;column-gap:calc(0.95rem);min-width:0;}
.step img{width:2.3rem;height:2.3rem;display:block;flex:none;}
.step span{
  font-size:1.5rem;font-weight:500;color:var(--ink);
  line-height:1.25;letter-spacing:-0.004em;
}

/* 6. footer */
.foot{
  margin-bottom:calc(2.2rem);
  font-size:1.0rem;font-weight:500;color:#6B7495;
  letter-spacing:0.012em;line-height:1.3;flex:none;
}

/* ---------- wide variant: horizontal steps, roomier type ---------- */
html.wide .headline{font-size:4.3rem;}
html.wide .steps{
  flex-direction:row;
  justify-content:space-between;
  column-gap:calc(2.2rem);
  align-items:center;
}
html.wide .step{
  flex:0 1 auto;
  align-items:center;
  column-gap:calc(0.8rem);
}
html.wide .step span{font-size:1.36rem;}
html.wide .cands{column-gap:calc(3rem);}

/* ---------- sky variant: narrow skyscraper ---------- */
html.sky body{row-gap:calc(1.35rem);}
html.sky .cands{column-gap:calc(0.7rem);}
html.sky .cand .nm{font-size:1.5rem;}
html.sky .cand .rl{font-size:0.98rem;letter-spacing:0.09em;margin-top:calc(0.3rem);}
html.sky .mono{font-size:1.8rem;width:4.6rem;height:4.6rem;}
html.sky .cta span{font-size:2.2rem;letter-spacing:0.04em;}
html.sky .cta{column-gap:calc(0.95rem);}
html.sky .cta img{height:2.2rem;}
html.sky .step span{font-size:1.62rem;}
html.sky .step img{width:2.6rem;height:2.6rem;}
html.sky .steps{row-gap:calc(1.25rem);}
html.sky .foot{font-size:1.3rem;margin-bottom:calc(1.9rem);}
html.sky .headline{margin-top:calc(2.4rem);}
html.sky .photo{min-height:calc(14rem);max-height:calc(54rem);}
</style>
</head>
<body>

  <h1 class="headline pad">Proven <em>RESULTS</em> for <em>WILLOWMERE</em> Families</h1>

  <div class="photo band">
    <img src="photo.png" alt="Willowmere neighborhood">
  </div>

  <div class="cands pad">
    <div class="cand"><div class="mono">DL</div><div class="nm">Dana Levi</div><div class="rl">Mayor</div></div>
    <div class="cand"><div class="mono">OH</div><div class="nm">Omar Haddad</div><div class="rl">Council</div></div>
    <div class="cand"><div class="mono">RC</div><div class="nm">Ruth Cohen</div><div class="rl">Council</div></div>
  </div>

  <div class="cta band">
    <img src="mail.svg" alt="">
    <span>VOTE BY MAIL</span>
  </div>

  <div class="steps pad">
    <div class="step"><img src="check.svg" alt=""><span>Request your ballot by October&nbsp;20</span></div>
    <div class="step"><img src="check.svg" alt=""><span>Fill it out at home</span></div>
    <div class="step"><img src="check.svg" alt=""><span>Mail it back by November&nbsp;3</span></div>
  </div>

  <div class="foot pad">Paid for by Willowmere Forward &middot; willowmereforward.org</div>

</body>
</html>
`;

const sizes = [
  {name:'portrait', w:1080, h:1350, s:1,    mode:'portrait'},
  {name:'wide',     w:1200, h:1000, s:0.85, mode:'wide'},
  {name:'sky',      w:300,  h:600,  s:0.28, mode:'sky'},
];

for (const cfg of sizes) {
  fs.writeFileSync(path.join(dir, cfg.name + '.html'), page(cfg));
}
console.log('wrote', sizes.map(s=>s.name+'.html').join(', '));

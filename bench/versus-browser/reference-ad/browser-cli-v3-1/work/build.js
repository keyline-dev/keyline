const fs = require('fs');
const path = require('path');
const dir = __dirname;

const SIZES = [
  { name: 'portrait', w: 1080, h: 1350, s: 1 },
  { name: 'wide', w: 1200, h: 1000, s: 0.85 },
  { name: 'sky', w: 300, h: 600, s: 0.28 },
];

const mail = fs.readFileSync(path.join(dir, 'mail.svg'), 'utf8').trim();
const check = fs.readFileSync(path.join(dir, 'check.svg'), 'utf8').trim();

const cands = [
  ['Dana Levi', 'Mayor'],
  ['Omar Haddad', 'Council'],
  ['Ruth Cohen', 'Council'],
];
const steps = [
  'Request your ballot by October 20',
  'Fill it out at home',
  'Mail it back by November 3',
];

function html({ w, h, s }) {
  const bw = w / s, bh = h / s; // design-box size in master units
  const wide = bw / bh > 1;     // short + wide box -> horizontal steps, tighter rhythm
  const tall = bw / bh < 0.65;  // very tall box -> let the photo band absorb extra height
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<style>
@font-face{font-family:'Inter';src:url('Inter.ttf') format('truetype');font-weight:100 900;font-style:normal;font-display:block;}
*{margin:0;padding:0;box-sizing:border-box;}
html,body{width:${w}px;height:${h}px;overflow:hidden;background:#fff;}
.stage{width:${w}px;height:${h}px;overflow:hidden;}
.page{
  width:${bw.toFixed(4)}px;height:${bh.toFixed(4)}px;
  transform:scale(${s});transform-origin:top left;
  background:#fff;font-family:'Inter',sans-serif;
  display:flex;flex-direction:column;align-items:stretch;
  color:#1B2A5C;
  -webkit-font-smoothing:antialiased;
}
.sp{flex:1 1 0;min-height:26px;max-height:190px;}
.sp.sm{max-height:120px;}
.pad{padding-left:76px;padding-right:76px;}

/* 1. headline */
.head{padding-top:64px;}
h1{font-size:76px;line-height:1.08;font-weight:800;letter-spacing:-1.6px;
   color:#1B2A5C;text-wrap:balance;}
h1 .r{color:#D0202E;}
.rule{margin-top:30px;width:148px;height:10px;background:#D0202E;border-radius:5px;}

/* 2. photo */
.photo{flex:4 1 auto;min-height:340px;max-height:600px;width:100%;overflow:hidden;position:relative;}
.photo img{width:100%;height:100%;object-fit:cover;object-position:42% 55%;display:block;}

/* 3. candidates */
.cands{display:flex;width:100%;gap:40px;}
.cand{flex:1 1 0;min-width:0;display:flex;flex-direction:column;align-items:center;
      text-align:center;padding:26px 8px 24px;border-top:6px solid #1B2A5C;}
.cand .nm{font-size:40px;font-weight:800;line-height:1.14;letter-spacing:-0.5px;white-space:nowrap;}
.cand .ro{margin-top:12px;font-size:25px;font-weight:700;color:#D0202E;
          letter-spacing:3.2px;text-transform:uppercase;white-space:nowrap;}

/* 4. CTA */
.cta{width:100%;background:#D0202E;display:flex;align-items:center;justify-content:center;
     gap:34px;padding:34px 40px;}
.cta svg{width:72px;height:56.6px;flex:none;display:block;}
.cta span{font-size:62px;font-weight:800;color:#fff;letter-spacing:4px;line-height:1;white-space:nowrap;}

/* 5. steps */
.steps{display:flex;flex-direction:column;gap:28px;width:100%;}
.step{display:flex;align-items:center;gap:26px;}
.step svg{width:48px;height:48px;flex:none;display:block;}
.step span{font-size:35px;font-weight:600;line-height:1.25;color:#1B2A5C;}

/* 6. footer */
.foot{padding-bottom:54px;padding-top:4px;}
.foot p{font-size:26px;font-weight:500;color:#6B7390;line-height:1.3;letter-spacing:0.2px;}

${tall ? `
/* very tall box: photo band takes the slack so gaps stay in rhythm */
.head{padding-top:84px;}
.photo{max-height:790px;}
.sp{max-height:132px;}
.sp.sm{max-height:104px;}
.steps{gap:34px;}
.foot{padding-bottom:70px;}
` : ''}
${wide ? `
/* short, wide box: tighten vertical rhythm and lay the steps out in a row */
.head{padding-top:46px;}
.rule{margin-top:22px;}
.photo{flex:6 1 auto;min-height:250px;max-height:350px;}
.sp{min-height:22px;max-height:96px;}
.sp.sm{max-height:70px;}
.cand{padding:22px 8px 20px;}
.cta{padding:30px 40px;}
.steps{flex-direction:row;align-items:flex-start;gap:48px;}
.step{flex:1 1 0;min-width:0;align-items:flex-start;gap:20px;}
.step svg{margin-top:1px;}
.foot{padding-bottom:44px;}
` : ''}
</style></head>
<body><div class="stage"><div class="page">

  <div class="head pad">
    <h1>Proven <span class="r">RESULTS</span> for <span class="r">WILLOWMERE</span> Families</h1>
    <div class="rule"></div>
  </div>

  <div class="sp"></div>

  <div class="photo"><img src="photo.jpg" alt=""></div>

  <div class="sp"></div>

  <div class="cands pad">
    ${cands.map(([n, r]) => `<div class="cand"><div class="nm">${n}</div><div class="ro">${r}</div></div>`).join('\n    ')}
  </div>

  <div class="sp"></div>

  <div class="cta">${mail}<span>VOTE BY MAIL</span></div>

  <div class="sp"></div>

  <div class="steps pad">
    ${steps.map(t => `<div class="step">${check}<span>${t}</span></div>`).join('\n    ')}
  </div>

  <div class="sp sm"></div>

  <div class="foot pad"><p>Paid for by Willowmere Forward &middot; willowmereforward.org</p></div>

</div></div></body></html>`;
}

for (const sz of SIZES) {
  fs.writeFileSync(path.join(dir, sz.name + '.html'), html(sz));
}
console.log('built');

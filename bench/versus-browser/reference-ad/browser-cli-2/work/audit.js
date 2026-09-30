const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
(async () => {
  const b = await chromium.launch();
  for (const [name, w, h] of [['portrait',1080,1350],['wide',1200,1000],['sky',300,600]]) {
    const p = await b.newPage({ viewport: { width: w, height: h } });
    await p.goto('file://' + process.cwd() + '/' + name + '.html');
    await p.waitForTimeout(400);
    const r = await p.evaluate(({w,h}) => {
      const out = { scroll: [document.documentElement.scrollWidth, document.documentElement.scrollHeight], bad: [], bands: [], rows: [] };
      document.querySelectorAll('.headline, .cand .name, .cand .role, .cta span, .step span, .footer').forEach(e => {
        const b = e.getBoundingClientRect();
        if (b.right > w + 0.6 || b.left < -0.6 || b.bottom > h + 0.6 || b.top < -0.6)
          out.bad.push(['outside', e.className || e.tagName, JSON.stringify(b)]);
        if (getComputedStyle(e).display !== 'inline' && (e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1))
          out.bad.push(['clipped', e.className || e.tagName, e.scrollWidth, e.clientWidth, e.scrollHeight, e.clientHeight]);
      });
      ['.photoband', '.cta'].forEach(s => { const b = document.querySelector(s).getBoundingClientRect(); out.bands.push([s, b.left, b.width]); });
      // sibling overlap check among flyer sections
      const secs = [...document.querySelector('.flyer').children].map(e => [e.className, e.getBoundingClientRect()]);
      for (let i = 1; i < secs.length; i++) if (secs[i][1].top < secs[i-1][1].bottom - 0.6) out.bad.push(['overlap', secs[i-1][0], secs[i][0]]);
      out.rows = secs.map(([c, b]) => [c, Math.round(b.top), Math.round(b.bottom)]);
      // column even spacing
      const cs = [...document.querySelectorAll('.cand')].map(e => e.getBoundingClientRect());
      out.cols = cs.map(b => [Math.round(b.left), Math.round(b.width)]);
      const names = [...document.querySelectorAll('.cand .name')].map(e => e.getBoundingClientRect());
      out.nameCenters = names.map((b,i) => Math.round(b.left + b.width/2 - (cs[i].left + cs[i].width/2)));
      // photo aspect (distortion check)
      const img = document.querySelector('.photoband img').getBoundingClientRect();
      out.photoBox = [Math.round(img.width), Math.round(img.height)];
      return out;
    }, {w,h});
    console.log(name, JSON.stringify(r, null, 1));
    await p.close();
  }
  await b.close();
})();

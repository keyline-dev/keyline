const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const D = __dirname;

(async () => {
  const b = await chromium.launch();
  for (const [name, w, h] of [['portrait',1080,1350],['wide',1200,1000],['sky',300,600]]) {
    const p = await b.newPage({ viewport: { width: w, height: h } });
    await p.goto(`file://${D}/${name}.html`);
    await p.evaluate(() => document.fonts.ready);
    const r = await p.evaluate(({w,h}) => {
      const out = { overflow: [], clipped: [], rects: {} };
      document.querySelectorAll('*').forEach(el => {
        const b = el.getBoundingClientRect();
        if (b.width === 0 && b.height === 0) return;
        const tag = el.className || el.tagName;
        // text overflow inside own box
        if (el.scrollWidth > el.clientWidth + 1 && el.clientWidth > 0)
          out.overflow.push(`${tag}: scrollW ${el.scrollWidth} > clientW ${el.clientWidth}`);
        if (el.scrollHeight > el.clientHeight + 1 && el.clientHeight > 0)
          out.overflow.push(`${tag}: scrollH ${el.scrollHeight} > clientH ${el.clientHeight}`);
        // outside viewport
        if (b.left < -0.5 || b.top < -0.5 || b.right > w + 0.5 || b.bottom > h + 0.5)
          out.clipped.push(`${tag}: ${JSON.stringify({l:+b.left.toFixed(1),t:+b.top.toFixed(1),r:+b.right.toFixed(1),bo:+b.bottom.toFixed(1)})}`);
      });
      const g = s => { const e = document.querySelector(s); if(!e) return null; const b = e.getBoundingClientRect();
        return {l:+b.left.toFixed(1),t:+b.top.toFixed(1),w:+b.width.toFixed(1),h:+b.height.toFixed(1),bo:+b.bottom.toFixed(1)}; };
      out.rects = { h1:g('h1'), photo:g('.photo'), cands:g('.cands'), cta:g('.cta'), steps:g('.steps'), foot:g('.foot') };
      out.cands = [...document.querySelectorAll('.cand')].map(c => {
        const cb = c.getBoundingClientRect();
        const nm = c.querySelector('.nm'), ro = c.querySelector('.ro');
        return { x:+cb.left.toFixed(1), w:+cb.width.toFixed(1),
                 nmW:+nm.scrollWidth, boxW:+nm.clientWidth, roW:+ro.scrollWidth };
      });
      const img = document.querySelector('.photo img');
      out.photoNat = { nw: img.naturalWidth, nh: img.naturalHeight,
                       dispW: +img.getBoundingClientRect().width.toFixed(1),
                       dispH: +img.getBoundingClientRect().height.toFixed(1),
                       fit: getComputedStyle(img).objectFit };
      out.ctaSpanW = document.querySelector('.cta span').scrollWidth;
      out.stepW = [...document.querySelectorAll('.step span')].map(s => s.scrollWidth);
      out.footW = document.querySelector('.foot').scrollWidth;
      out.bodyScroll = { sw: document.documentElement.scrollWidth, sh: document.documentElement.scrollHeight };
      return out;
    }, {w,h});
    console.log(`\n===== ${name} ${w}x${h} =====`);
    console.log('overflow:', r.overflow.length ? r.overflow : 'none');
    console.log('out-of-viewport:', r.clipped.length ? r.clipped : 'none');
    console.log('doc scroll:', r.bodyScroll);
    console.log('rects:', JSON.stringify(r.rects, null, 0));
    console.log('cands:', JSON.stringify(r.cands));
    console.log('photo:', JSON.stringify(r.photoNat));
    console.log('ctaSpanW:', r.ctaSpanW, 'stepW:', r.stepW, 'footW:', r.footW);
    await p.close();
  }
  await b.close();
})();

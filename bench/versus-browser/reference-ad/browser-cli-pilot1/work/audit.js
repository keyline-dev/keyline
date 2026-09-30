const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const sizes = [
  { name: 'portrait', w: 1080, h: 1350 },
  { name: 'wide', w: 1200, h: 1000 },
  { name: 'sky', w: 300, h: 600 },
];

(async () => {
  const browser = await chromium.launch();
  for (const z of sizes) {
    const page = await browser.newPage({ viewport: { width: z.w, height: z.h }, deviceScaleFactor: 1 });
    await page.goto('file://' + path.resolve(z.name + '.html'));
    await page.waitForLoadState('networkidle');
    const r = await page.evaluate((vp) => {
      const out = { issues: [], boxes: {} };
      const els = [...document.querySelectorAll('h1, .rule, .photo, .cands, .cand, .badge, .name, .role, .cta, .cta span, .cta img, .steps, .step, .step span, .step img, .foot')];
      for (const el of els) {
        const b = el.getBoundingClientRect();
        const key = el.className || el.tagName;
        // horizontal text overflow
        if (el.scrollWidth > Math.ceil(el.clientWidth) + 1 && el.clientWidth > 0)
          out.issues.push(`X-overflow: ${key} scrollW=${el.scrollWidth} clientW=${el.clientWidth}`);
        if (el.scrollHeight > Math.ceil(el.clientHeight) + 1 && el.clientHeight > 0)
          out.issues.push(`Y-overflow: ${key} scrollH=${el.scrollHeight} clientH=${el.clientHeight}`);
        if (b.left < -0.5 || b.right > vp.w + 0.5)
          out.issues.push(`out-of-frame-x: ${key} L=${b.left.toFixed(1)} R=${b.right.toFixed(1)}`);
        if (b.top < -0.5 || b.bottom > vp.h + 0.5)
          out.issues.push(`out-of-frame-y: ${key} T=${b.top.toFixed(1)} B=${b.bottom.toFixed(1)}`);
      }
      // section stacking / overlap
      const secs = [...document.querySelectorAll('.page > *')];
      for (let i = 1; i < secs.length; i++) {
        const a = secs[i - 1].getBoundingClientRect(), c = secs[i].getBoundingClientRect();
        if (c.top < a.bottom - 0.5)
          out.issues.push(`overlap: ${secs[i-1].className} bottom=${a.bottom.toFixed(1)} vs ${secs[i].className} top=${c.top.toFixed(1)}`);
        out.boxes[secs[i].className.split(' ')[0]] = `${c.top.toFixed(1)}-${c.bottom.toFixed(1)} h=${c.height.toFixed(1)}`;
      }
      const h = document.querySelector('.hero').getBoundingClientRect();
      out.boxes.hero = `${h.top.toFixed(1)}-${h.bottom.toFixed(1)} h=${h.height.toFixed(1)}`;
      // full-bleed bands
      for (const sel of ['.photo', '.cta']) {
        const b = document.querySelector(sel).getBoundingClientRect();
        if (Math.abs(b.left) > 0.5 || Math.abs(b.width - vp.w) > 0.5)
          out.issues.push(`not-full-bleed: ${sel} L=${b.left} W=${b.width}`);
      }
      // even column spacing
      const cx = [...document.querySelectorAll('.cand')].map(e => { const b = e.getBoundingClientRect(); return (b.left + b.right) / 2; });
      out.boxes.colCenters = cx.map(v => v.toFixed(1)).join(', ');
      const g1 = cx[1] - cx[0], g2 = cx[2] - cx[1];
      if (Math.abs(g1 - g2) > 0.6) out.issues.push(`uneven columns: ${g1.toFixed(2)} vs ${g2.toFixed(2)}`);
      // photo aspect (no distortion) - object-fit cover is inherently non-distorting; verify img fills band
      const im = document.querySelector('.photo img').getBoundingClientRect();
      const pb = document.querySelector('.photo').getBoundingClientRect();
      if (Math.abs(im.width - pb.width) > 0.5 || Math.abs(im.height - pb.height) > 0.5)
        out.issues.push(`photo img not filling band`);
      out.boxes.headlineLines = Math.round(document.querySelector('h1').getBoundingClientRect().height / parseFloat(getComputedStyle(document.querySelector('h1')).lineHeight));
      out.boxes.font = document.fonts.check('700 20px Inter');
      return out;
    }, z);
    console.log(`\n=== ${z.name} (${z.w}x${z.h}) ===`);
    console.log(JSON.stringify(r.boxes, null, 1));
    console.log(r.issues.length ? 'ISSUES:\n  ' + r.issues.join('\n  ') : 'no issues');
    await page.close();
  }
  await browser.close();
})();

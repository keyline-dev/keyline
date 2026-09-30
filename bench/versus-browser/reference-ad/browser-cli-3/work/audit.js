// Audits each rendered size for overflow, clipping, overlap and band width.
const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');
const HERE = __dirname;

const SIZES = [
  ['portrait', 1080, 1350],
  ['wide', 1200, 1000],
  ['sky', 300, 600],
];

(async () => {
  const browser = await chromium.launch();
  let bad = 0;
  for (const [name, w, h] of SIZES) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.goto('file://' + path.join(HERE, name + '.html'));
    await page.evaluate(() => document.fonts.ready);
    const r = await page.evaluate(({ w, h }) => {
      const issues = [];
      const de = document.documentElement;
      if (de.scrollWidth > w) issues.push(`page scrollWidth ${de.scrollWidth} > ${w}`);
      if (de.scrollHeight > h) issues.push(`page scrollHeight ${de.scrollHeight} > ${h}`);

      // every leaf-ish element inside the viewport
      const els = [...document.querySelectorAll('.flyer *')];
      for (const el of els) {
        const b = el.getBoundingClientRect();
        if (b.width === 0 && b.height === 0) continue;
        const tag = el.className || el.tagName;
        if (b.left < -0.5 || b.top < -0.5 || b.right > w + 0.5 || b.bottom > h + 0.5)
          issues.push(`OUT-OF-BOUNDS ${tag}: ${JSON.stringify({l:+b.left.toFixed(1),t:+b.top.toFixed(1),r:+b.right.toFixed(1),bo:+b.bottom.toFixed(1)})}`);
        // text clipped by its own box?
        if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow !== 'visible')
          issues.push(`CLIPPED-X ${tag}: ${el.scrollWidth} > ${el.clientWidth}`);
        if (el.children.length === 0 && el.textContent.trim()) {
          if (el.scrollWidth > el.clientWidth + 1)
            issues.push(`TEXT-OVERFLOW-X ${tag}: scroll ${el.scrollWidth} vs client ${el.clientWidth}`);
        }
      }

      // full-width bands
      for (const sel of ['.photo', '.cta']) {
        const b = document.querySelector(sel).getBoundingClientRect();
        if (Math.abs(b.left) > 0.5 || Math.abs(b.width - w) > 0.5)
          issues.push(`BAND ${sel} not full width: left=${b.left} width=${b.width}`);
      }

      // candidate columns evenly spaced / no overlap
      const cands = [...document.querySelectorAll('.cand')].map(c => c.getBoundingClientRect());
      const widths = cands.map(c => +c.width.toFixed(2));
      const gaps = [cands[1].left - cands[0].right, cands[2].left - cands[1].right].map(g => +g.toFixed(2));
      if (Math.max(...widths) - Math.min(...widths) > 0.6) issues.push(`columns uneven widths ${widths}`);
      if (Math.abs(gaps[0] - gaps[1]) > 0.6) issues.push(`columns uneven gaps ${gaps}`);
      if (gaps.some(g => g < 1)) issues.push(`columns touching ${gaps}`);
      // names must not collide with column edges
      for (const nm of document.querySelectorAll('.cand .nm, .cand .rl')) {
        const p = nm.parentElement.getBoundingClientRect();
        const b = nm.getBoundingClientRect();
        if (b.width > p.width + 0.5) issues.push(`"${nm.textContent}" (${b.width.toFixed(1)}) wider than column (${p.width.toFixed(1)})`);
      }

      // sibling vertical overlap inside the flyer
      const kids = [...document.querySelector('.flyer').children];
      for (let i = 0; i < kids.length - 1; i++) {
        const a = kids[i].getBoundingClientRect(), c = kids[i + 1].getBoundingClientRect();
        if (c.top < a.bottom - 0.5) issues.push(`OVERLAP ${kids[i].className} / ${kids[i+1].className}`);
      }

      const img = document.querySelector('.photo img');
      const pb = document.querySelector('.photo').getBoundingClientRect();
      return { issues, photo: { w: +pb.width.toFixed(0), h: +pb.height.toFixed(0) },
               natural: [img.naturalWidth, img.naturalHeight],
               gaps, widths,
               fonts: getComputedStyle(document.querySelector('h1')).fontFamily,
               interLoaded: document.fonts.check('700 40px Inter') };
    }, { w, h });

    console.log(`\n=== ${name} ${w}x${h} ===`);
    console.log(`  photo band ${r.photo.w}x${r.photo.h} (img ${r.natural.join('x')})  Inter loaded: ${r.interLoaded}`);
    console.log(`  column widths ${r.widths}  gaps ${r.gaps}`);
    if (r.issues.length) { bad += r.issues.length; r.issues.forEach(i => console.log('  ✗ ' + i)); }
    else console.log('  ✓ no layout defects');
    await page.close();
  }
  await browser.close();
  console.log(bad ? `\n${bad} issue(s)` : '\nALL CLEAN');
})();

const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const DIR = __dirname;
const sizes = [
  { name: 'portrait', W: 1080, H: 1350 },
  { name: 'wide', W: 1200, H: 1000 },
  { name: 'sky', W: 300, H: 600 },
];

(async () => {
  const browser = await chromium.launch();
  for (const sz of sizes) {
    const page = await browser.newPage({ viewport: { width: sz.W, height: sz.H } });
    await page.goto(`file://${DIR}/${sz.name}.html`);
    await page.evaluate(() => document.fonts.ready);
    const r = await page.evaluate(() => {
      const out = { issues: [], rows: [] };
      const stage = document.querySelector('.stage');
      const SB = stage.getBoundingClientRect();
      const f = document.querySelector('.flyer');
      // vertical overflow of flyer content
      const fr = f.getBoundingClientRect();
      const boxes = [];
      document.querySelectorAll('.flyer > *').forEach(el => {
        const b = el.getBoundingClientRect();
        boxes.push({ cls: el.className, top: +b.top.toFixed(1), bottom: +b.bottom.toFixed(1),
                     left: +b.left.toFixed(1), right: +b.right.toFixed(1) });
      });
      out.rows = boxes;
      out.stage = { w: +SB.width.toFixed(1), h: +SB.height.toFixed(1),
                    top: +SB.top.toFixed(1), bottom: +SB.bottom.toFixed(1) };
      // section overlap / out of bounds
      for (let i = 0; i < boxes.length; i++) {
        const b = boxes[i];
        if (b.bottom > SB.bottom + 0.6) out.issues.push(`${b.cls} overflows bottom by ${(b.bottom - SB.bottom).toFixed(1)}`);
        if (b.top < SB.top - 0.6) out.issues.push(`${b.cls} overflows top`);
        if (i > 0 && b.top < boxes[i-1].bottom - 0.6) out.issues.push(`${b.cls} overlaps ${boxes[i-1].cls}`);
      }
      // full-bleed bands
      ['.photo', '.cta'].forEach(sel => {
        const b = document.querySelector(sel).getBoundingClientRect();
        if (Math.abs(b.width - SB.width) > 0.6 || Math.abs(b.left - SB.left) > 0.6)
          out.issues.push(`${sel} not full width (${b.width.toFixed(1)} vs ${SB.width.toFixed(1)})`);
      });
      // photo not distorted
      const img = document.querySelector('.photo img');
      out.photoBand = { w: +img.getBoundingClientRect().width.toFixed(0), h: +img.getBoundingClientRect().height.toFixed(0) };
      if (getComputedStyle(img).objectFit !== 'cover') out.issues.push('photo not object-fit:cover');
      // text clipping: every text element must fit inside its scroll box
      document.querySelectorAll('h1, p, .nm, .rl, .badge, .cta span, .footer').forEach(el => {
        if (el.scrollWidth > el.clientWidth + 1) out.issues.push(`text clipped horizontally: "${el.textContent.trim().slice(0,30)}"`);
        // tolerance of 4px: Inter's ascender/descender metrics exceed the line box slightly
        if (el.scrollHeight > el.clientHeight + 4) out.issues.push(`text clipped vertically: "${el.textContent.trim().slice(0,30)}"`);
      });
      // candidate columns evenly spaced
      const cs = [...document.querySelectorAll('.cand')].map(e => e.getBoundingClientRect());
      const centers = cs.map(b => b.left + b.width / 2);
      const g1 = centers[1] - centers[0], g2 = centers[2] - centers[1];
      out.candGaps = [ +g1.toFixed(1), +g2.toFixed(1) ];
      if (Math.abs(g1 - g2) > 1) out.issues.push(`candidate columns uneven: ${g1.toFixed(1)} vs ${g2.toFixed(1)}`);
      // steps count + check icons
      out.steps = document.querySelectorAll('.step').length;
      if (document.querySelectorAll('.step svg').length !== 3) out.issues.push('missing check icons');
      // steps must not overlap each other
      const st = [...document.querySelectorAll('.step')].map(e => e.getBoundingClientRect());
      for (let i = 1; i < st.length; i++) {
        const a = st[i-1], b = st[i];
        const ov = !(b.left >= a.right - 0.6 || b.top >= a.bottom - 0.6 || b.right <= a.left + 0.6 || b.bottom <= a.top + 0.6);
        if (ov) out.issues.push(`step ${i+1} overlaps step ${i}`);
      }
      // font actually applied
      out.font = getComputedStyle(document.querySelector('h1')).fontFamily;
      out.h1Lines = +(document.querySelector('h1').getBoundingClientRect().height /
                      parseFloat(getComputedStyle(document.querySelector('h1')).lineHeight)).toFixed(2);
      out.bottomSlack = +(SB.bottom - boxes[boxes.length-1].bottom).toFixed(1);
      return out;
    });
    const fontOk = await page.evaluate(() => document.fonts.check('700 74px Inter'));
    console.log(`\n=== ${sz.name} (${sz.W}x${sz.H}) ===`);
    console.log('  stage:', JSON.stringify(r.stage), ' photoBand:', JSON.stringify(r.photoBand));
    console.log('  candGaps:', r.candGaps, ' steps:', r.steps, ' h1 lines:', r.h1Lines, ' Inter loaded:', fontOk);
    console.log('  last section bottom slack:', r.bottomSlack);
    console.log(r.issues.length ? '  ISSUES:\n    - ' + r.issues.join('\n    - ') : '  OK: no issues');
    await page.close();
  }
  await browser.close();
})();

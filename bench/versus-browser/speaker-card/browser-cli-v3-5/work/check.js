const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const SIZES = [
  ['square', 1080, 1080],
  ['landscape', 1920, 1080],
  ['story', 1080, 1920],
];

(async () => {
  const browser = await chromium.launch();
  for (const [name, width, height] of SIZES) {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
    await page.goto('file://' + path.resolve('card.html'));
    await page.waitForLoadState('networkidle');
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(300);

    const report = await page.evaluate(() => {
      const out = { variant: document.documentElement.className, items: [], problems: [] };
      const vw = innerWidth, vh = innerHeight;
      const sel = ['.brand', '.event', '.brand img', '.eyebrow', '.title', '.avatar:not(.avatar-hero)',
                   '.avatar-hero', '.name', '.role', '.meta', '.btn', '.hairline'];
      const boxes = [];
      for (const s of sel) {
        document.querySelectorAll(s).forEach((el) => {
          const cs = getComputedStyle(el);
          if (cs.display === 'none' || !el.offsetParent && cs.position !== 'fixed') {
            if (cs.display === 'none') return;
          }
          const r = el.getBoundingClientRect();
          if (r.width === 0 && r.height === 0) return;
          out.items.push({ s, x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) });
          boxes.push({ s, el, r });
          // viewport overflow
          if (r.left < -0.5 || r.top < -0.5 || r.right > vw + 0.5 || r.bottom > vh + 0.5) {
            out.problems.push(`OVERFLOW ${s}: ${JSON.stringify({ l: r.left, t: r.top, r: r.right, b: r.bottom })}`);
          }
          // clipped text
          if (el.scrollHeight > el.clientHeight + 1 && cs.overflow !== 'visible') {
            out.problems.push(`CLIPPED-V ${s}`);
          }
          if (el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== 'visible') {
            out.problems.push(`CLIPPED-H ${s}`);
          }
        });
      }
      // circle check
      document.querySelectorAll('.avatar').forEach((el) => {
        const cs = getComputedStyle(el);
        if (cs.display === 'none') return;
        const r = el.getBoundingClientRect();
        if (Math.abs(r.width - r.height) > 0.5) out.problems.push(`NOT-CIRCLE ${Math.round(r.width)}x${Math.round(r.height)}`);
        const img = el.querySelector('img');
        const ir = img.getBoundingClientRect();
        if (Math.abs(ir.width - ir.height) > 0.5) out.problems.push(`IMG-NOT-SQUARE ${Math.round(ir.width)}x${Math.round(ir.height)}`);
      });
      // pairwise overlap of text/content boxes
      const inter = (a, b) => {
        const x = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        return x > 2 && y > 2 ? Math.round(x) + 'x' + Math.round(y) : null;
      };
      for (let i = 0; i < boxes.length; i++) {
        for (let j = i + 1; j < boxes.length; j++) {
          const A = boxes[i], B = boxes[j];
          if (A.el.contains(B.el) || B.el.contains(A.el)) continue;
          if (A.s === '.brand' || B.s === '.brand') continue; // container of logo+event
          const ov = inter(A.r, B.r);
          if (ov) out.problems.push(`OVERLAP ${A.s} <> ${B.s} (${ov})`);
        }
      }
      return out;
    });
    console.log('=== ' + name + ' (' + width + 'x' + height + ') variant=' + report.variant);
    for (const it of report.items) console.log('   ', it.s.padEnd(28), `x=${it.x} y=${it.y} w=${it.w} h=${it.h}`);
    console.log(report.problems.length ? '  PROBLEMS:\n   - ' + report.problems.join('\n   - ') : '  OK: no geometry problems');
    await page.screenshot({ path: name + '.png' });
    await page.close();
  }
  await browser.close();
})();

const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const cases = [
  ['portrait', 1080, 1350],
  ['wide', 1200, 1000],
  ['sky', 300, 600],
];

(async () => {
  const browser = await chromium.launch();
  for (const [name, w, h] of cases) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.goto('file://' + path.resolve(__dirname, name + '.html'));
    await page.waitForTimeout(400);
    const r = await page.evaluate((vh) => {
      const out = { sections: [], problems: [] };
      const sel = ['.header', '.photoband', '.candidates', '.cta', '.steps', '.footer'];
      let prevBottom = null, prevName = null;
      for (const s of sel) {
        const el = document.querySelector(s);
        const b = el.getBoundingClientRect();
        out.sections.push({ s, top: +b.top.toFixed(1), bottom: +b.bottom.toFixed(1), h: +b.height.toFixed(1) });
        if (prevBottom !== null && b.top < prevBottom - 0.6) out.problems.push(`overlap ${prevName}/${s}`);
        prevBottom = b.bottom; prevName = s;
      }
      const last = out.sections[out.sections.length - 1];
      if (last.bottom > vh + 0.6) out.problems.push(`overflow bottom ${last.bottom}`);
      if (last.bottom < vh - 0.6) out.problems.push(`gap at bottom ${vh - last.bottom}`);
      // text clipping checks
      document.querySelectorAll('h1, .eyebrow, .name, .role, .txt, .step span, .footer').forEach((el) => {
        if (el.scrollWidth > el.clientWidth + 1) out.problems.push(`h-clip "${el.textContent.trim().slice(0, 24)}" ${el.scrollWidth}>${el.clientWidth}`);
        const b = el.getBoundingClientRect();
        if (b.right > window.innerWidth + 0.5 || b.left < -0.5 || b.bottom > vh + 0.5 || b.top < -0.5)
          out.problems.push(`oob "${el.textContent.trim().slice(0, 24)}" ${JSON.stringify({l:+b.left.toFixed(1),r:+b.right.toFixed(1),t:+b.top.toFixed(1),bo:+b.bottom.toFixed(1)})}`);
        // check clipped by ancestor overflow hidden
        let p = el.parentElement;
        while (p) {
          if (getComputedStyle(p).overflow === 'hidden') {
            const pb = p.getBoundingClientRect();
            if (b.right > pb.right + 0.5 || b.left < pb.left - 0.5 || b.bottom > pb.bottom + 0.5 || b.top < pb.top - 0.5)
              out.problems.push(`clipped-by-${p.className} "${el.textContent.trim().slice(0, 20)}"`);
          }
          p = p.parentElement;
        }
      });
      // candidate column spacing evenness
      const cands = [...document.querySelectorAll('.cand')].map((c) => c.getBoundingClientRect());
      if (cands.length === 3) {
        const gaps = [cands[1].left - cands[0].right, cands[2].left - cands[1].right];
        const widths = cands.map((c) => +c.width.toFixed(1));
        out.cols = { gaps: gaps.map((g) => +g.toFixed(1)), widths };
        if (Math.abs(gaps[0] - gaps[1]) > 0.6) out.problems.push('uneven column gaps');
        if (Math.max(...widths) - Math.min(...widths) > 0.6) out.problems.push('uneven column widths');
      }
      // full-width bands
      ['.photoband', '.cta', '.footer'].forEach((s) => {
        const b = document.querySelector(s).getBoundingClientRect();
        if (Math.abs(b.left) > 0.5 || Math.abs(b.right - window.innerWidth) > 0.5) out.problems.push(`band ${s} not full width`);
      });
      // photo aspect (no distortion)
      const img = document.querySelector('.photoband img');
      out.photo = { natural: `${img.naturalWidth}x${img.naturalHeight}`, box: `${img.clientWidth}x${img.clientHeight}`, fit: getComputedStyle(img).objectFit };
      // font loaded?
      out.fontLoaded = document.fonts.check('800 62px Inter');
      return out;
    }, h);
    console.log('==== ' + name + ` (${w}x${h}) ====`);
    console.log(JSON.stringify(r, null, 1));
    await page.close();
  }
  await browser.close();
})();

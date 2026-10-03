const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const SIZES = [
  ['square', 1080, 1080],
  ['landscape', 1920, 1080],
  ['story', 1080, 1920],
];

(async () => {
  const browser = await chromium.launch();
  let bad = 0;
  for (const [name, W, H] of SIZES) {
    const page = await browser.newPage({ viewport: { width: W, height: H } });
    await page.goto('file://' + path.resolve(__dirname, name + '.html'));
    await page.waitForTimeout(400);
    const report = await page.evaluate(({ W, H }) => {
      const out = { overflow: [], overlap: [], circle: null, scroll: null };
      const de = document.documentElement;
      out.scroll = { sw: de.scrollWidth, sh: de.scrollHeight };

      const sel = '.header, .event-name, .portrait, .speaker-name, .speaker-role, .kicker, .title, .meta, .btn';
      const els = [...document.querySelectorAll(sel)];

      for (const el of els) {
        const r = el.getBoundingClientRect();
        const id = el.className + ' "' + (el.textContent || '').trim().slice(0, 24) + '"';
        if (r.left < -0.5 || r.top < -0.5 || r.right > W + 0.5 || r.bottom > H + 0.5) {
          out.overflow.push({ id, box: [r.left, r.top, r.right, r.bottom] });
        }
        // clipped content inside its own box (only matters if it actually clips)
        const ov = getComputedStyle(el);
        const clips = ov.overflowX !== 'visible' || ov.overflowY !== 'visible';
        if (clips && (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1)) {
          out.overflow.push({ id: id + ' [self-clip]', sw: el.scrollWidth, cw: el.clientWidth, sh: el.scrollHeight, ch: el.clientHeight });
        }
      }

      // pairwise overlap of text/visual leaves
      const leaves = [...document.querySelectorAll('.event-name, .portrait, .speaker-name, .speaker-role, .kicker, .title, .meta, .btn')];
      for (let i = 0; i < leaves.length; i++) {
        for (let j = i + 1; j < leaves.length; j++) {
          const a = leaves[i], b = leaves[j];
          if (a.contains(b) || b.contains(a)) continue;
          const ra = a.getBoundingClientRect(), rb = b.getBoundingClientRect();
          const ox = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left);
          const oy = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
          if (ox > 1 && oy > 1) {
            out.overlap.push({ a: a.className, b: b.className, ox: +ox.toFixed(1), oy: +oy.toFixed(1) });
          }
        }
      }

      const p = document.querySelector('.portrait').getBoundingClientRect();
      out.circle = { w: +p.width.toFixed(2), h: +p.height.toFixed(2), round: Math.abs(p.width - p.height) < 0.5 };

      // title line count
      const t = document.querySelector('.title');
      const cs = getComputedStyle(t);
      out.titleLines = Math.round(t.getBoundingClientRect().height / parseFloat(cs.lineHeight));
      out.titleFont = cs.fontFamily + ' / ' + cs.fontSize;
      return out;
    }, { W, H });

    const ok = !report.overflow.length && !report.overlap.length && report.circle.round &&
               report.scroll.sw <= W && report.scroll.sh <= H;
    if (!ok) bad++;
    console.log(`\n=== ${name} ${W}x${H} === ${ok ? 'PASS' : 'FAIL'}`);
    console.log('  scroll:', JSON.stringify(report.scroll), ' circle:', JSON.stringify(report.circle));
    console.log('  title:', report.titleFont, 'lines:', report.titleLines);
    if (report.overflow.length) console.log('  OVERFLOW:', JSON.stringify(report.overflow, null, 1));
    if (report.overlap.length) console.log('  OVERLAP:', JSON.stringify(report.overlap, null, 1));
    await page.close();
  }
  await browser.close();
  console.log(bad ? `\n${bad} size(s) failing` : '\nAll sizes pass');
})();

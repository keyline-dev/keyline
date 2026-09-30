// Geometry audit: overflow, clipping, overlap and circle-roundness checks.
const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');

const SIZES = {
  square: [1080, 1080],
  landscape: [1920, 1080],
  story: [1080, 1920],
};

(async () => {
  const browser = await chromium.launch();
  let bad = 0;

  for (const [size, [w, h]] of Object.entries(SIZES)) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.goto(`file://${__dirname}/card.html?size=${size}`);
    await page.waitForFunction(() => document.fonts.ready.then(() => true));
    await page.waitForTimeout(250);

    const report = await page.evaluate(({ w, h }) => {
      const sel = {
        header: '.header', logo: '.header img', event: '.eventname',
        avatar: '.avatar', name: '.name', role: '.role',
        title: '.title', meta: '.meta', cta: '.cta',
      };
      const out = { issues: [], boxes: {} };
      const els = {};
      for (const [k, s] of Object.entries(sel)) {
        const el = document.querySelector(s);
        if (!el) { out.issues.push(`MISSING ${k}`); continue; }
        els[k] = el;
        const r = el.getBoundingClientRect();
        out.boxes[k] = { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };

        // inside viewport?
        if (r.left < -0.5 || r.top < -0.5 || r.right > w + 0.5 || r.bottom > h + 0.5)
          out.issues.push(`OUT OF BOUNDS ${k}: ${JSON.stringify(out.boxes[k])}`);

        // text clipped by its own box?
        if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow !== 'visible')
          out.issues.push(`CLIPPED-X ${k}: scroll ${el.scrollWidth} > client ${el.clientWidth}`);
        if (el.scrollHeight > el.clientHeight + 1 && getComputedStyle(el).overflow !== 'visible')
          out.issues.push(`CLIPPED-Y ${k}: scroll ${el.scrollHeight} > client ${el.clientHeight}`);
      }

      // circle must be square-ish and fully round
      const a = els.avatar;
      if (a) {
        const r = a.getBoundingClientRect();
        if (Math.abs(r.width - r.height) > 0.5) out.issues.push(`AVATAR NOT SQUARE ${r.width}x${r.height}`);
        const br = getComputedStyle(a).borderRadius;
        if (!/50%|9999|999px/.test(br)) out.issues.push(`AVATAR RADIUS ${br}`);
        const img = a.querySelector('img');
        const ir = img.getBoundingClientRect();
        if (ir.width < r.width - 0.5 || ir.height < r.height - 0.5)
          out.issues.push(`AVATAR IMG UNDERFILLS ${ir.width}x${ir.height} in ${r.width}x${r.height}`);
        if (getComputedStyle(img).objectFit !== 'cover') out.issues.push('AVATAR IMG NOT COVER');
      }

      // pairwise overlap of visible content blocks
      const keys = ['header', 'avatar', 'name', 'role', 'title', 'meta', 'cta'];
      const rect = k => els[k] && els[k].getBoundingClientRect();
      for (let i = 0; i < keys.length; i++) {
        for (let j = i + 1; j < keys.length; j++) {
          const A = rect(keys[i]), B = rect(keys[j]);
          if (!A || !B) continue;
          const ox = Math.min(A.right, B.right) - Math.max(A.left, B.left);
          const oy = Math.min(A.bottom, B.bottom) - Math.max(A.top, B.top);
          if (ox > 1 && oy > 1) out.issues.push(`OVERLAP ${keys[i]} / ${keys[j]} by ${ox.toFixed(0)}x${oy.toFixed(0)}px`);
        }
      }

      // page-level scroll overflow
      const de = document.documentElement;
      if (de.scrollWidth > w + 1 || de.scrollHeight > h + 1)
        out.issues.push(`PAGE OVERFLOW ${de.scrollWidth}x${de.scrollHeight} vs ${w}x${h}`);

      // font actually applied
      out.font = getComputedStyle(els.title).fontFamily;
      out.titleLines = Math.round(els.title.getBoundingClientRect().height / parseFloat(getComputedStyle(els.title).lineHeight));

      // dead space: biggest vertical gap between stacked blocks
      const rs = keys.map(k => rect(k)).filter(Boolean).sort((a, b) => a.top - b.top);
      let maxGap = 0, gapAt = '';
      for (let i = 1; i < rs.length; i++) {
        const g = rs[i].top - Math.max(...rs.slice(0, i).map(r => r.bottom));
        if (g > maxGap) { maxGap = g; gapAt = `${i}`; }
      }
      out.maxGap = Math.round(maxGap);
      return out;
    }, { w, h });

    console.log(`\n=== ${size} (${w}x${h}) ===`);
    console.log('font:', report.font, '| title lines:', report.titleLines, '| largest vertical gap:', report.maxGap + 'px');
    console.log('avatar:', JSON.stringify(report.boxes.avatar), 'cta:', JSON.stringify(report.boxes.cta));
    if (report.issues.length === 0) console.log('OK — no issues');
    else { bad += report.issues.length; report.issues.forEach(i => console.log('  ✗ ' + i)); }
    await page.close();
  }

  await browser.close();
  console.log(bad === 0 ? '\nALL CHECKS PASSED' : `\n${bad} ISSUE(S)`);
})();

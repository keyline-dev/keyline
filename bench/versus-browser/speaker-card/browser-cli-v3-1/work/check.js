const { chromium } = require('playwright');
const path = require('path');

const SIZES = { square: [1080, 1080], landscape: [1920, 1080], story: [1080, 1920] };

(async () => {
  const browser = await chromium.launch();
  let bad = 0;
  for (const [name, [w, h]] of Object.entries(SIZES)) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.goto('file://' + path.resolve(name + '.html'));
    await page.waitForTimeout(400);
    const r = await page.evaluate(({ w, h }) => {
      const sel = ['.head', '.logo svg', '.event', '.ring', '.photo', '.name', '.role', '.title', '.meta', '.btn'];
      const out = { overflow: [], clipped: [], overlaps: [], circle: null, lines: {}, boxes: {} };
      const els = [];
      for (const s of sel) {
        document.querySelectorAll(s).forEach(e => {
          const b = e.getBoundingClientRect();
          out.boxes[s] = { x: +b.x.toFixed(1), y: +b.y.toFixed(1), w: +b.width.toFixed(1), h: +b.height.toFixed(1) };
          if (b.left < -0.5 || b.top < -0.5 || b.right > w + 0.5 || b.bottom > h + 0.5)
            out.overflow.push({ s, ...out.boxes[s] });
          // real clipping: element actually hides overflow AND content exceeds it meaningfully
          const ov = getComputedStyle(e);
          const hides = /hidden|clip|auto|scroll/.test(ov.overflowX + ov.overflowY);
          if (hides && (e.scrollWidth > e.clientWidth + 2 || e.scrollHeight > e.clientHeight + 2))
            out.clipped.push({ s, sw: e.scrollWidth, cw: e.clientWidth, sh: e.scrollHeight, ch: e.clientHeight });
          if (ov.textOverflow === 'ellipsis' || ov.webkitLineClamp !== 'none')
            out.clipped.push({ s, reason: 'truncation style' });
          els.push({ s, b });
        });
      }
      // text line counts via Range rects
      for (const s of ['.title', '.name', '.role', '.meta', '.event', '.btn']) {
        const e = document.querySelector(s);
        if (!e) continue;
        // measure only real text nodes, cluster by vertical centre
        const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT);
        const rects = [];
        let n;
        while ((n = walker.nextNode())) {
          if (!n.textContent.trim()) continue;
          const rg = document.createRange(); rg.selectNodeContents(n);
          for (const r of rg.getClientRects()) if (r.height > 2 && r.width > 0) rects.push(r);
        }
        const fs = parseFloat(getComputedStyle(e).fontSize);
        const centres = [];
        for (const r of rects) {
          const cy = r.top + r.height / 2;
          if (!centres.some(c => Math.abs(c - cy) < fs * 0.6)) centres.push(cy);
        }
        out.lines[s] = centres.length;
        // widow check: width of last visual line vs widest line
        if (centres.length > 1) {
          const last = Math.max(...centres);
          const lastRects = rects.filter(r => Math.abs(r.top + r.height / 2 - last) < fs * 0.6);
          const lastW = Math.max(...lastRects.map(r => r.right)) - Math.min(...lastRects.map(r => r.left));
          const widest = Math.max(...rects.map(r => r.width));
          out.lines[s + '_lastLineFrac'] = +(lastW / widest).toFixed(2);
        }
        out.lines[s + '_maxRight'] = +Math.max(...rects.map(r => r.right)).toFixed(1);
      }
      // circle check
      const ring = document.querySelector('.ring').getBoundingClientRect();
      const ph = document.querySelector('.photo').getBoundingClientRect();
      out.circle = {
        ringSquare: Math.abs(ring.width - ring.height) < 0.5,
        photoSquare: Math.abs(ph.width - ph.height) < 0.5,
        ringR: getComputedStyle(document.querySelector('.ring')).borderRadius,
        photoR: getComputedStyle(document.querySelector('.photo')).borderRadius,
      };
      // pairwise overlap of text/visual blocks
      const T = ['.head', '.ring', '.name', '.role', '.title', '.meta', '.btn'];
      const list = els.filter(e => T.includes(e.s));
      for (let i = 0; i < list.length; i++)
        for (let j = i + 1; j < list.length; j++) {
          const a = list[i].b, b = list[j].b;
          const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left);
          const oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
          if (ox > 1 && oy > 1) out.overlaps.push({ a: list[i].s, b: list[j].s, ox: +ox.toFixed(1), oy: +oy.toFixed(1) });
        }
      return out;
    }, { w, h });

    const probs = [];
    if (r.overflow.length) probs.push('OVERFLOW ' + JSON.stringify(r.overflow));
    if (r.clipped.length) probs.push('CLIPPED ' + JSON.stringify(r.clipped));
    if (r.overlaps.length) probs.push('OVERLAP ' + JSON.stringify(r.overlaps));
    if (!r.circle.ringSquare || !r.circle.photoSquare) probs.push('CIRCLE NOT SQUARE ' + JSON.stringify(r.circle));
    if (r.lines['.name'] > 1) probs.push('name wraps to ' + r.lines['.name'] + ' lines');
    if (r.lines['.meta'] > 1) probs.push('meta wraps to ' + r.lines['.meta'] + ' lines');
    if (r.lines['.btn'] > 1) probs.push('btn wraps');
    const lf = r.lines['.title_lastLineFrac'];
    if (lf !== undefined && lf < 0.22) probs.push('title widow: last line only ' + lf + ' of line width');

    console.log(`\n=== ${name} ${w}x${h} ===`);
    console.log('title lines:', r.lines['.title'], '| name lines:', r.lines['.name'], '| role lines:', r.lines['.role']);
    console.log('title last-line fill:', r.lines['.title_lastLineFrac']);
    console.log('title box:', JSON.stringify(r.boxes['.title']), 'text maxRight:', r.lines['.title_maxRight']);
    console.log('ring:', JSON.stringify(r.boxes['.ring']), 'btn:', JSON.stringify(r.boxes['.btn']));
    const bottomGap = h - (r.boxes['.btn'].y + r.boxes['.btn'].h);
    console.log('bottom gap under button:', bottomGap.toFixed(1));
    if (probs.length) { bad++; console.log('PROBLEMS:'); probs.forEach(p => console.log('  ! ' + p)); }
    else console.log('OK — no overflow / clipping / overlap');
    await page.close();
  }
  await browser.close();
  process.exit(bad ? 1 : 0);
})();

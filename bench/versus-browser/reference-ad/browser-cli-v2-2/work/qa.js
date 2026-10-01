// Geometry QA: checks for viewport overflow, horizontal text clipping,
// band full-bleed, even column spacing and element overlap.
const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');
const dir = __dirname;

const sizes = [
  { name: 'portrait', w: 1080, h: 1350 },
  { name: 'wide', w: 1200, h: 1000 },
  { name: 'sky', w: 300, h: 600 },
];

(async () => {
  const browser = await chromium.launch();
  let bad = 0;
  for (const s of sizes) {
    const page = await browser.newPage({ viewport: { width: s.w, height: s.h } });
    await page.goto('file://' + path.join(dir, s.name + '.html'));
    await page.evaluate(() => document.fonts.ready);
    const r = await page.evaluate(({ w, h }) => {
      const out = { issues: [], boxes: {} };
      const de = document.documentElement;
      if (de.scrollHeight > h + 0.5) out.issues.push(`page scrollHeight ${de.scrollHeight} > ${h}`);
      if (de.scrollWidth > w + 0.5) out.issues.push(`page scrollWidth ${de.scrollWidth} > ${w}`);

      const all = [...document.querySelectorAll('body *')];
      for (const el of all) {
        const b = el.getBoundingClientRect();
        if (b.width === 0 && b.height === 0) continue;
        if (b.top < -0.5 || b.bottom > h + 0.5 || b.left < -0.5 || b.right > w + 0.5) {
          out.issues.push(`${el.className || el.tagName} out of frame: ${JSON.stringify({t:+b.top.toFixed(1),r:+b.right.toFixed(1),b:+b.bottom.toFixed(1),l:+b.left.toFixed(1)})}`);
        }
        // clipping of text inside block boxes (skip inline boxes: no client metrics)
        const disp = getComputedStyle(el).display;
        if (el.children.length === 0 && el.textContent.trim() && disp !== 'inline') {
          if (el.scrollWidth > el.clientWidth + 1) out.issues.push(`text clipped horizontally: "${el.textContent.trim().slice(0,28)}" (${el.scrollWidth}>${el.clientWidth})`);
          if (el.scrollHeight > el.clientHeight + 1) out.issues.push(`text clipped vertically: "${el.textContent.trim().slice(0,28)}" (${el.scrollHeight}>${el.clientHeight})`);
        }
      }
      // bands full bleed
      for (const sel of ['.photo', '.cta']) {
        const b = document.querySelector(sel).getBoundingClientRect();
        if (Math.abs(b.left) > 0.5 || Math.abs(b.width - w) > 0.5) out.issues.push(`${sel} not full width: left=${b.left} width=${b.width}`);
      }
      // candidate columns evenly spaced (centers)
      const cs = [...document.querySelectorAll('.cand')].map(e => { const b = e.getBoundingClientRect(); return b.left + b.width / 2; });
      const d1 = cs[1] - cs[0], d2 = cs[2] - cs[1];
      if (Math.abs(d1 - d2) > 1) out.issues.push(`candidate columns uneven: ${d1.toFixed(1)} vs ${d2.toFixed(1)}`);
      // sibling overlap among top level sections
      const secs = [...document.body.children].map(e => ({ n: e.className, b: e.getBoundingClientRect() }));
      for (let i = 0; i < secs.length - 1; i++) {
        if (secs[i].b.bottom > secs[i + 1].b.top + 0.5) out.issues.push(`overlap: ${secs[i].n} / ${secs[i + 1].n}`);
      }
      // overlap within rows (steps, candidates)
      const hit = (a, b) => a.right > b.left + 0.5 && b.right > a.left + 0.5 && a.bottom > b.top + 0.5 && b.bottom > a.top + 0.5;
      for (const [sel, nm] of [['.step', 'steps'], ['.cand', 'cands']]) {
        const es = [...document.querySelectorAll(sel)].map(e => e.getBoundingClientRect());
        for (let i = 0; i < es.length; i++) for (let j = i + 1; j < es.length; j++) if (hit(es[i], es[j])) out.issues.push(`overlap in ${nm}: ${i}/${j}`);
      }
      // text runs must not collide with any other text/icon box
      const leaves = [...document.querySelectorAll('.nm,.rl,.mono,.step span,.step img,.cta span,.cta img,.foot,.headline')].map(e => ({ n: e.className || e.tagName, b: e.getBoundingClientRect() }));
      for (let i = 0; i < leaves.length; i++) for (let j = i + 1; j < leaves.length; j++) if (hit(leaves[i].b, leaves[j].b)) out.issues.push(`collide: ${leaves[i].n} / ${leaves[j].n}`);
      // icon + label overlap inside flex rows
      for (const st of document.querySelectorAll('.step, .cta')) {
        const im = st.querySelector('img').getBoundingClientRect();
        const tx = st.querySelector('span').getBoundingClientRect();
        if (im.right > tx.left + 0.5) out.issues.push('icon/text overlap');
      }
      const g = (sel) => { const e = document.querySelector(sel); const b = e.getBoundingClientRect(); return { t: +b.top.toFixed(1), b: +b.bottom.toFixed(1), h: +b.height.toFixed(1) }; };
      out.boxes = { headline: g('.headline'), photo: g('.photo'), cands: g('.cands'), cta: g('.cta'), steps: g('.steps'), foot: g('.foot') };
      out.photoImg = (() => { const i = document.querySelector('.photo img'); return { nat: i.naturalWidth / i.naturalHeight, box: +(i.clientWidth / i.clientHeight).toFixed(3), fit: getComputedStyle(i).objectFit }; })();
      out.font = getComputedStyle(document.querySelector('.headline')).fontFamily;
      out.fontLoaded = document.fonts.check('700 40px Inter');
      return out;
    }, s);
    console.log(`\n=== ${s.name} (${s.w}x${s.h}) ===`);
    console.log('font:', r.font, 'loaded:', r.fontLoaded, 'photo fit:', r.photoImg.fit);
    console.log('boxes:', JSON.stringify(r.boxes));
    if (r.issues.length) { bad += r.issues.length; r.issues.forEach(i => console.log('  ISSUE:', i)); }
    else console.log('  OK — no geometry issues');
    await page.close();
  }
  await browser.close();
  console.log(bad ? `\n${bad} issue(s)` : '\nALL CLEAN');
})();

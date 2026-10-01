const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const URL = 'file://' + path.resolve(__dirname, 'card.html');
const SIZES = [
  ['square', 1080, 1080],
  ['landscape', 1920, 1080],
  ['story', 1080, 1920],
];

(async () => {
  const browser = await chromium.launch();
  let bad = 0;

  for (const [name, width, height] of SIZES) {
    const page = await browser.newPage({ viewport: { width, height } });
    await page.goto(URL);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(250);

    const report = await page.evaluate(({ width, height }) => {
      const sel = {
        brand: '.brand', logo: '.brand img', event: '.event',
        avatar: '.avatar', name: '.name', role: '.role',
        eyebrow: '.eyebrow', title: '.title', meta: '.meta', btn: '.btn',
      };
      const out = { issues: [], boxes: {}, lines: {} };
      const els = {};
      for (const [k, s] of Object.entries(sel)) {
        const el = document.querySelector(s);
        if (!el) { out.issues.push(`MISSING ${k}`); continue; }
        els[k] = el;
        const r = el.getBoundingClientRect();
        out.boxes[k] = { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };

        // out of frame
        if (r.left < -0.5 || r.top < -0.5 || r.right > width + 0.5 || r.bottom > height + 0.5) {
          out.issues.push(`OUT-OF-FRAME ${k}: [${r.left.toFixed(1)},${r.top.toFixed(1)},${r.right.toFixed(1)},${r.bottom.toFixed(1)}] vs ${width}x${height}`);
        }
        // content clipped inside its own box (tolerate <=4px line-height/descender rounding)
        if (el.scrollWidth > Math.ceil(r.width) + 4 || el.scrollHeight > Math.ceil(r.height) + 4) {
          out.issues.push(`CLIPPED ${k}: scroll ${el.scrollWidth}x${el.scrollHeight} vs box ${Math.round(r.width)}x${Math.round(r.height)}`);
        }
        // real line count = box height / line-height
        if (['name','role','title','meta','event'].includes(k)) {
          const lh = parseFloat(getComputedStyle(el).lineHeight);
          out.lines[k] = Math.round(r.height / lh);
        }
      }

      // circle check on avatar
      if (els.avatar) {
        const r = els.avatar.getBoundingClientRect();
        const cs = getComputedStyle(els.avatar);
        if (Math.abs(r.width - r.height) > 0.5) out.issues.push(`AVATAR-NOT-SQUARE ${r.width}x${r.height}`);
        const brRaw = cs.borderTopLeftRadius;
        const br = brRaw.includes('%') ? (parseFloat(brRaw) / 100) * r.width : parseFloat(brRaw);
        if (br < r.width / 2 - 0.5) out.issues.push(`AVATAR-NOT-CIRCLE radius ${brRaw} -> ${br.toFixed(1)}px (need ${(r.width/2).toFixed(1)})`);
        // intrinsic aspect must be square-croppable
        out.avatarNatural = `${els.avatar.naturalWidth}x${els.avatar.naturalHeight}`;
        if (cs.objectFit !== 'cover') out.issues.push(`AVATAR-OBJECT-FIT ${cs.objectFit}`);
      }

      // pairwise overlap of visible text/graphic elements
      const keys = Object.keys(els);
      const pad = 0;
      for (let i = 0; i < keys.length; i++) {
        for (let j = i + 1; j < keys.length; j++) {
          const a = keys[i], b = keys[j];
          if (els[a].contains(els[b]) || els[b].contains(els[a])) continue;
          const ra = els[a].getBoundingClientRect(), rb = els[b].getBoundingClientRect();
          const ox = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left) - pad;
          const oy = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top) - pad;
          if (ox > 1 && oy > 1) out.issues.push(`OVERLAP ${a} x ${b} by ${ox.toFixed(1)}x${oy.toFixed(1)}`);
        }
      }

      // document overflow
      if (document.documentElement.scrollWidth > width + 1 || document.documentElement.scrollHeight > height + 1) {
        out.issues.push(`DOC-OVERFLOW ${document.documentElement.scrollWidth}x${document.documentElement.scrollHeight}`);
      }

      // font actually applied
      out.font = getComputedStyle(document.querySelector('.title')).fontFamily;
      out.interLoaded = document.fonts.check('700 60px Inter');

      // free space between body content and footer / brand
      const brand = els.brand.getBoundingClientRect();
      const foot = document.querySelector('.footer').getBoundingClientRect();
      // union bbox of all real body content
      const parts = [...document.querySelectorAll('.avatar,.name,.role,.eyebrow,.title')].map(e => e.getBoundingClientRect());
      const top = Math.min(...parts.map(r => r.top));
      const bottom = Math.max(...parts.map(r => r.bottom));
      out.space = {
        brandToContent: +(top - brand.bottom).toFixed(1),
        contentToFooter: +(foot.top - bottom).toFixed(1),
      };
      return out;
    }, { width, height });

    console.log(`\n=== ${name} ${width}x${height} ===`);
    console.log('font:', report.font, '| Inter loaded:', report.interLoaded);
    console.log('lines:', JSON.stringify(report.lines));
    console.log('space:', JSON.stringify(report.space));
    console.log('avatar:', JSON.stringify(report.boxes.avatar));
    if (report.issues.length) { bad += report.issues.length; report.issues.forEach(i => console.log('  !!', i)); }
    else console.log('  OK - no geometry defects');
    await page.close();
  }

  await browser.close();
  console.log(bad ? `\nTOTAL ISSUES: ${bad}` : '\nALL CLEAN');
})();

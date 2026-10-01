const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const SIZES = {
  square: [1080, 1080],
  landscape: [1920, 1080],
  story: [1080, 1920],
};

(async () => {
  const browser = await chromium.launch();
  let bad = 0;
  for (const [name, [w, h]] of Object.entries(SIZES)) {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.goto('file://' + path.join(__dirname, name + '.html'));
    await page.waitForSelector('body.ready');

    const report = await page.evaluate(({ w, h }) => {
      const issues = [];
      const boxes = [];
      const sel = '.brand, .brand img, .event, .avatar, .who, .name, .role, .eyebrow, .title, .meta, .cta, .badge';
      document.querySelectorAll(sel).forEach((el) => {
        const r = el.getBoundingClientRect();
        const label = el.className || el.tagName;
        // viewport clipping
        if (r.left < -0.5 || r.top < -0.5 || r.right > w + 0.5 || r.bottom > h + 0.5) {
          issues.push(`CLIPPED ${label}: ${JSON.stringify({l:+r.left.toFixed(1),t:+r.top.toFixed(1),r:+r.right.toFixed(1),b:+r.bottom.toFixed(1)})}`);
        }
        // internal text overflow
        if (el.scrollWidth > el.clientWidth + 1 && el.clientWidth > 0) {
          issues.push(`H-OVERFLOW ${label}: scrollW=${el.scrollWidth} clientW=${el.clientWidth}`);
        }
        if (el.scrollHeight > el.clientHeight + 1 && el.clientHeight > 0) {
          issues.push(`V-OVERFLOW ${label}: scrollH=${el.scrollHeight} clientH=${el.clientHeight}`);
        }
        if (['title','name','role','meta','event','badge'].some(c => (el.className||'').split(' ').includes(c)) || el.classList.contains('cta') || el.classList.contains('avatar')) {
          boxes.push({ label: (el.className||'').split(' ')[0], r });
        }
      });

      // pairwise overlap of leaf text/graphic blocks
      for (let i = 0; i < boxes.length; i++) {
        for (let j = i + 1; j < boxes.length; j++) {
          const a = boxes[i].r, b = boxes[j].r;
          const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left);
          const oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
          if (ox > 1 && oy > 1) {
            issues.push(`OVERLAP ${boxes[i].label} <-> ${boxes[j].label} (${ox.toFixed(1)}x${oy.toFixed(1)})`);
          }
        }
      }

      // circle check
      const av = document.querySelector('.avatar').getBoundingClientRect();
      if (Math.abs(av.width - av.height) > 0.5) issues.push(`AVATAR NOT SQUARE ${av.width}x${av.height}`);
      const img = document.querySelector('.avatar img');
      if (getComputedStyle(img).objectFit !== 'cover') issues.push('AVATAR IMG NOT COVER');

      // document scroll
      if (document.documentElement.scrollWidth > w + 1) issues.push('PAGE H-SCROLL ' + document.documentElement.scrollWidth);
      if (document.documentElement.scrollHeight > h + 1) issues.push('PAGE V-SCROLL ' + document.documentElement.scrollHeight);

      return {
        issues,
        titleFont: getComputedStyle(document.getElementById('title')).fontSize,
        titleFamily: getComputedStyle(document.getElementById('title')).fontFamily,
        fontLoaded: document.fonts.check('700 60px Inter'),
        avatar: `${av.width}x${av.height}`,
      };
    }, { w, h });

    console.log(`\n=== ${name} ${w}x${h} ===`);
    console.log(`  title ${report.titleFont} (${report.titleFamily}) | Inter loaded: ${report.fontLoaded} | avatar ${report.avatar}`);
    if (report.issues.length === 0) console.log('  OK — no clipping, overflow or overlap');
    else { bad += report.issues.length; report.issues.forEach(i => console.log('  !! ' + i)); }

    await page.screenshot({ path: path.join(__dirname, name + '.png') });
    await page.close();
  }
  await browser.close();
  console.log('\nTotal issues: ' + bad);
})();

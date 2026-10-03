const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const sizes = [
  { name: 'portrait', w: 1080, h: 1350 },
  { name: 'wide', w: 1200, h: 1000 },
  { name: 'sky', w: 300, h: 600 },
];
(async () => {
  const b = await chromium.launch();
  for (const z of sizes) {
    const p = await b.newPage({ viewport: { width: z.w, height: z.h } });
    await p.goto('file://' + __dirname + '/' + z.name + '.html');
    await p.waitForTimeout(400);
    const r = await p.evaluate(({ w, h }) => {
      const out = { overflow: [], boxes: [] };
      const doc = document.documentElement;
      out.scroll = [doc.scrollWidth, doc.scrollHeight];
      document.querySelectorAll('body *').forEach((el) => {
        const b = el.getBoundingClientRect();
        if (b.width === 0 && b.height === 0) return;
        if (b.left < -0.5 || b.top < -0.5 || b.right > w + 0.5 || b.bottom > h + 0.5)
          out.overflow.push([el.className || el.tagName, +b.left.toFixed(1), +b.top.toFixed(1), +b.right.toFixed(1), +b.bottom.toFixed(1)]);
        // text clipping check
        if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflow !== 'visible')
          out.overflow.push(['CLIP-' + el.className, el.scrollWidth, el.clientWidth]);
      });
      const sel = ['.headline h1', '.photo', '.cands', '.cta', '.steplist', '.footer span'];
      sel.forEach((s) => {
        const e = document.querySelector(s);
        const b = e.getBoundingClientRect();
        out.boxes.push([s, +b.top.toFixed(1), +b.bottom.toFixed(1), +b.left.toFixed(1), +b.width.toFixed(1)]);
      });
      out.cands = [...document.querySelectorAll('.cand .nm')].map((e) => {
        const b = e.getBoundingClientRect();
        return [+(b.left + b.width / 2).toFixed(1), +b.width.toFixed(1), e.scrollWidth > e.clientWidth + 0.5];
      });
      const bodyKids = [...document.body.children];
      out.lastBottom = bodyKids[bodyKids.length - 1].getBoundingClientRect().bottom;
      const img = document.querySelector('.photo img');
      out.imgBox = [img.clientWidth, img.clientHeight, img.naturalWidth, img.naturalHeight];
      out.font = document.fonts.check('700 20px Inter');
      return out;
    }, z);
    console.log('==', z.name, JSON.stringify(r, null, 1));
    await p.close();
  }
  await b.close();
})();

import { chromium } from '/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright/index.mjs';

const D = '/private/var/folders/hf/35wnqrnj2bbb1_llbg87zlt40000gn/T/vs-59274c54b484acb7';
const sizes = [
  ['portrait', 1080, 1350],
  ['wide', 1200, 1000],
  ['sky', 300, 600],
];

const browser = await chromium.launch();
for (const [name, W, H] of sizes) {
  const page = await browser.newPage({ viewport: { width: W, height: H } });
  await page.goto(`file://${D}/${name}.html`);
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => document.fonts.ready);

  const r = await page.evaluate(({ W, H }) => {
    const out = { overflow: [], outside: [], overlap: [], bands: [], cols: [], photo: null, gaps: [] };
    const els = [...document.querySelectorAll('body *')];
    for (const el of els) {
      const b = el.getBoundingClientRect();
      if (b.width === 0 && b.height === 0) continue;
      const tag = el.className || el.tagName;
      // horizontal / vertical content overflow
      if (el.scrollWidth - el.clientWidth > 1 || el.scrollHeight - el.clientHeight > 1) {
        out.overflow.push(`${tag} scroll ${el.scrollWidth}x${el.scrollHeight} vs client ${el.clientWidth}x${el.clientHeight}`);
      }
      if (b.left < -0.5 || b.top < -0.5 || b.right > W + 0.5 || b.bottom > H + 0.5) {
        out.outside.push(`${tag} ${JSON.stringify({l:+b.left.toFixed(1),t:+b.top.toFixed(1),r:+b.right.toFixed(1),bo:+b.bottom.toFixed(1)})}`);
      }
    }
    // full-width bands
    for (const sel of ['.photoband', '.ctabar']) {
      const b = document.querySelector(sel).getBoundingClientRect();
      out.bands.push(`${sel} x:${b.left.toFixed(1)}->${b.right.toFixed(1)} h:${b.height.toFixed(1)}`);
    }
    // column spacing
    const cs = [...document.querySelectorAll('.cand')].map(c => {
      const b = c.getBoundingClientRect(); return { c: (b.left + b.right) / 2, w: b.width };
    });
    out.cols = cs.map(c => c.c.toFixed(1));
    out.colGaps = [cs[1].c - cs[0].c, cs[2].c - cs[1].c].map(v => v.toFixed(2));
    // photo aspect integrity
    const img = document.querySelector('.photoband img');
    const ib = img.getBoundingClientRect();
    out.photo = { box: `${ib.width.toFixed(1)}x${ib.height.toFixed(1)}`, natural: `${img.naturalWidth}x${img.naturalHeight}`, fit: getComputedStyle(img).objectFit };
    // sibling vertical overlap among block sections
    const secs = [...document.querySelectorAll('.flyer > *')].filter(e => !e.classList.contains('gap'));
    for (let i = 0; i < secs.length - 1; i++) {
      const a = secs[i].getBoundingClientRect(), b2 = secs[i + 1].getBoundingClientRect();
      const g = b2.top - a.bottom;
      out.gaps.push(`${secs[i].className}->${secs[i+1].className}: ${g.toFixed(1)}`);
      if (g < -0.5) out.overlap.push(`${secs[i].className} / ${secs[i+1].className} overlap ${g.toFixed(1)}`);
    }
    const last = secs[secs.length - 1].getBoundingClientRect();
    out.gaps.push(`bottom margin: ${(H - last.bottom).toFixed(1)}`);
    out.gaps.unshift(`top margin: ${secs[0].getBoundingClientRect().top.toFixed(1)}`);
    out.headlineLines = document.querySelector('.headline').getClientRects().length;
    return out;
  }, { W, H });

  console.log(`\n=== ${name} ${W}x${H} ===`);
  console.log('overflow :', r.overflow.length ? r.overflow : 'none');
  console.log('outside  :', r.outside.length ? r.outside : 'none');
  console.log('overlap  :', r.overlap.length ? r.overlap : 'none');
  console.log('bands    :', r.bands.join(' | '));
  console.log('col ctrs :', r.cols.join(', '), ' gaps:', r.colGaps.join(' / '));
  console.log('photo    :', JSON.stringify(r.photo));
  console.log('gaps     :', r.gaps.join('  |  '));
  await page.close();
}
await browser.close();

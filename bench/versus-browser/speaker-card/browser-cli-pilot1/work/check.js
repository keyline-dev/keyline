const { chromium } = require('/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright');
const path = require('path');

const sizes = [['square',1080,1080],['landscape',1920,1080],['story',1080,1920]];

(async () => {
  const b = await chromium.launch();
  let bad = 0;
  for (const [n,W,H] of sizes) {
    const p = await b.newPage({ viewport:{width:W,height:H} });
    await p.goto('file://' + path.resolve(n + '.html'));
    await p.waitForTimeout(400);
    const r = await p.evaluate((V) => {
      const out = { overflowDoc:[], outOfBounds:[], overlaps:[], clipped:[], circle:null, title:null };
      const [W,H] = V;
      if (document.documentElement.scrollWidth > W) out.overflowDoc.push('docW=' + document.documentElement.scrollWidth);
      if (document.documentElement.scrollHeight > H) out.overflowDoc.push('docH=' + document.documentElement.scrollHeight);
      document.querySelectorAll('.col').forEach((c,i) => {
        if (c.scrollHeight > c.clientHeight + 0.5) out.overflowDoc.push('col' + i + ' H ' + c.scrollHeight + '>' + c.clientHeight);
        if (c.scrollWidth > c.clientWidth + 0.5) out.overflowDoc.push('col' + i + ' W ' + c.scrollWidth + '>' + c.clientWidth);
      });
      const sel = '.ev,.nm,.rl,.title,.meta span:last-child,.cta,.kicker,.pf,.brand img';
      const items = [...document.querySelectorAll(sel)].map(e => {
        const r = e.getBoundingClientRect();
        return { e, r, k: (e.className && e.className.baseVal === undefined ? e.className : e.tagName) + ':' + (e.textContent || '').slice(0,18) };
      });
      for (const it of items) {
        const { r, k } = it;
        if (r.left < -0.5 || r.top < -0.5 || r.right > W + 0.5 || r.bottom > H + 0.5)
          out.outOfBounds.push(k + ' ' + JSON.stringify({l:Math.round(r.left),t:Math.round(r.top),rt:Math.round(r.right),b:Math.round(r.bottom)}));
        // text clipped inside its own box?
        if (it.e.scrollHeight > it.e.clientHeight + 1 && getComputedStyle(it.e).overflow !== 'visible')
          out.clipped.push(k);
      }
      for (let i=0;i<items.length;i++) for (let j=i+1;j<items.length;j++) {
        const a=items[i], c=items[j];
        if (a.e.contains(c.e) || c.e.contains(a.e)) continue;
        const ox = Math.min(a.r.right,c.r.right) - Math.max(a.r.left,c.r.left);
        const oy = Math.min(a.r.bottom,c.r.bottom) - Math.max(a.r.top,c.r.top);
        if (ox > 2 && oy > 2) out.overlaps.push(a.k + ' <> ' + c.k + ' (' + Math.round(ox) + 'x' + Math.round(oy) + ')');
      }
      const pf = document.querySelector('.pf').getBoundingClientRect();
      const cs = getComputedStyle(document.querySelector('.pf'));
      out.circle = { w: Math.round(pf.width), h: Math.round(pf.height), radius: cs.borderRadius, square: Math.abs(pf.width-pf.height) < 0.5 };
      const t = document.querySelector('.title');
      const st = getComputedStyle(t);
      out.title = { size: st.fontSize, lines: Math.round(t.getBoundingClientRect().height / parseFloat(st.lineHeight)) };
      return out;
    }, [W,H]);
    const errs = [...r.overflowDoc.map(x=>'OVERFLOW '+x), ...r.outOfBounds.map(x=>'OUT '+x), ...r.overlaps.map(x=>'OVERLAP '+x), ...r.clipped.map(x=>'CLIPPED '+x)];
    if (!r.circle.square) errs.push('PORTRAIT NOT SQUARE ' + JSON.stringify(r.circle));
    bad += errs.length;
    console.log('\n=== ' + n + ' ' + W + 'x' + H + ' ===');
    console.log('  circle:', JSON.stringify(r.circle), ' title:', JSON.stringify(r.title));
    console.log(errs.length ? errs.map(e=>'  ✗ '+e).join('\n') : '  ✓ no defects');
    await p.close();
  }
  await b.close();
  console.log('\nTOTAL DEFECTS: ' + bad);
})();

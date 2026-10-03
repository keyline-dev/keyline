window.__audit = function () {
  {
    const card = document.getElementById('card');
    const cb = card.getBoundingClientRect();
    const cs = getComputedStyle(card);
    const pad = {
      t: parseFloat(cs.paddingTop), r: parseFloat(cs.paddingRight),
      b: parseFloat(cs.paddingBottom), l: parseFloat(cs.paddingLeft)
    };
    const inner = { l: cb.left + pad.l, r: cb.right - pad.r, t: cb.top + pad.t, b: cb.bottom - pad.b };
    const problems = [];

    // --- 1. text glyph boxes must sit inside the padding box ---
    const textEls = ['.brand .ename', '.sname', '.srole', '.eyebrow', '.title', '.meta', '.cta'];
    const boxes = {};
    for (const s of textEls) {
      const e = document.querySelector(s);
      if (!e) { problems.push(s + ' MISSING'); continue; }
      const rng = document.createRange();
      rng.selectNodeContents(e);
      const rects = [...rng.getClientRects()].filter(r => r.width > 0 && r.height > 0);
      if (!rects.length) { problems.push(s + ' has no rendered text'); continue; }
      const gl = Math.min(...rects.map(r => r.left));
      const gr = Math.max(...rects.map(r => r.right));
      const gt = Math.min(...rects.map(r => r.top));
      const gb = Math.max(...rects.map(r => r.bottom));
      boxes[s] = { l: gl, r: gr, t: gt, b: gb };
      const eps = 1;
      if (gl < inner.l - eps) problems.push(`${s} text overflows LEFT by ${(inner.l - gl).toFixed(1)}px`);
      if (gr > inner.r + eps) problems.push(`${s} text overflows RIGHT by ${(gr - inner.r).toFixed(1)}px`);
      if (gt < inner.t - eps) problems.push(`${s} text overflows TOP by ${(inner.t - gt).toFixed(1)}px`);
      if (gb > inner.b + eps) problems.push(`${s} text overflows BOTTOM by ${(gb - inner.b).toFixed(1)}px`);
      // ellipsis / hidden-clip detection
      const ecs = getComputedStyle(e);
      if (ecs.textOverflow === 'ellipsis' && e.scrollWidth > e.clientWidth + 1)
        problems.push(s + ' is truncated with an ellipsis');
      if (ecs.overflow !== 'visible' && (e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1))
        problems.push(s + ' content is clipped by overflow');
    }

    // --- 2. avatar must be a true circle, fully inside the card ---
    const av = document.querySelector('.avatar');
    const ar = av.getBoundingClientRect();
    const acs = getComputedStyle(av);
    if (Math.abs(ar.width - ar.height) > 0.5)
      problems.push(`avatar not square: ${ar.width}x${ar.height}`);
    const br = parseFloat(acs.borderTopLeftRadius);
    if (!(acs.borderTopLeftRadius.includes('%') ? parseFloat(acs.borderTopLeftRadius) >= 50 : br >= ar.width / 2 - 0.5))
      problems.push('avatar border-radius is not a full circle: ' + acs.borderTopLeftRadius);
    const img = av.querySelector('img');
    if (getComputedStyle(img).objectFit !== 'cover')
      problems.push('portrait img object-fit is not cover (would distort)');
    if (img.naturalWidth && Math.abs(img.getBoundingClientRect().width - img.getBoundingClientRect().height) > 0.5)
      problems.push('portrait img box is not square');
    const rw = parseFloat(acs.getPropertyValue('--ringw')) || 6;
    const pad2 = rw + 10; // ring + halo must stay on canvas
    if (ar.left - pad2 < cb.left + 2 || ar.right + pad2 > cb.right - 2 ||
        ar.top - pad2 < cb.top + 2 || ar.bottom + pad2 > cb.bottom - 2)
      problems.push('avatar (incl. ring) touches/exceeds the card edge');

    // --- 3. no overlap between the main blocks ---
    const blockSel = ['.brand', '.avatar', '.who', '.eyebrow', '.title', '.footer', '#rightCol > div'];
    // the avatar paints a violet ring + halo outside its border box, so grow
    // its rect by that amount before testing collisions / clearance
    const ringW = parseFloat(getComputedStyle(av).getPropertyValue('--ringw')) || 6;
    const halo = ringW + 10;
    const blocks = blockSel
      .map(s => ({ s, e: document.querySelector(s) }))
      .filter(o => o.e && o.e.getBoundingClientRect().width > 0)
      .map(o => {
        const r = o.e.getBoundingClientRect();
        const g = o.s === '.avatar' ? halo : 0;
        return { s: o.s, r: { left: r.left - g, right: r.right + g, top: r.top - g, bottom: r.bottom + g } };
      });
    for (let i = 0; i < blocks.length; i++) {
      for (let j = i + 1; j < blocks.length; j++) {
        const a = blocks[i].r, b = blocks[j].r;
        if (blocks[i].e === blocks[j].e) continue;
        const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        if (ox > 1 && oy > 1)
          problems.push(`OVERLAP ${blocks[i].s} <-> ${blocks[j].s} (${Math.round(ox)}x${Math.round(oy)}px)`);
        // horizontally aligned but vertically crammed together
        else if (ox > 1 && oy > -12)
          problems.push(`TOO TIGHT ${blocks[i].s} <-> ${blocks[j].s} (${Math.round(-oy)}px apart)`);
      }
    }
    // meta vs cta in the footer row
    if (boxes['.meta'] && boxes['.cta']) {
      const m = boxes['.meta'], c = boxes['.cta'];
      const ox = Math.min(m.r, c.r) - Math.max(m.l, c.l);
      const oy = Math.min(m.b, c.b) - Math.max(m.t, c.t);
      if (ox > 1 && oy > 1) problems.push('OVERLAP meta <-> cta');
      if (ox <= 1 && oy > 1 && Math.max(m.l, c.l) - Math.min(m.r, c.r) < 24)
        problems.push('meta and cta are closer than 24px');
    }

    // --- 4. card fills the viewport exactly ---
    if (Math.round(cb.width) !== window.innerWidth || Math.round(cb.height) !== window.innerHeight)
      problems.push(`card ${Math.round(cb.width)}x${Math.round(cb.height)} != viewport ${window.innerWidth}x${window.innerHeight}`);

    const t = document.querySelector('.title');
    const lh = parseFloat(getComputedStyle(t).lineHeight);
    const tb = boxes['.title'];

    return {
      variant: document.body.className,
      fontLoaded: document.fonts.check('700 60px Inter') && [...document.fonts].some(f => f.family === 'Inter' && f.status === 'loaded'),
      titleFontSize: getComputedStyle(t).fontSize,
      titleLines: Math.round((tb.b - tb.t) / lh),
      avatarDiameter: Math.round(ar.width),
      slackBelowTitle: Math.round(document.querySelector('.footer').getBoundingClientRect().top - tb.b),
      bottomSlack: Math.round(inner.b - document.querySelector('.footer').getBoundingClientRect().bottom),
      problems
    };
  }
};

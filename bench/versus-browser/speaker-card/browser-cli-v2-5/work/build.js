const fs = require('fs');
const path = require('path');

const EVENT = 'FIELDNOTES CONF 2026';
const NAME = 'Maya Okonkwo-Lindqvist';
const ROLE = 'Principal Engineer, Northwind Labs';
const TITLE = 'Shipping at the speed of trust: what ten years of on-call taught us about resilient systems';
const DATE = 'November&nbsp;14,&nbsp;2026';
const VENUE = 'Harbor&nbsp;Hall,&nbsp;Lisbon';

const brand = `
      <header class="brand">
        <img src="logo.svg" alt="Fieldnotes Conf logo">
        <span class="rule"></span>
        <span class="event">${EVENT}</span>
      </header>`;

const speaker = `
      <section class="speaker">
        <div class="avatar"><img src="portrait.png" alt="${NAME}"></div>
        <div class="who">
          <div class="name">${NAME}</div>
          <div class="role">${ROLE}</div>
        </div>
      </section>`;

const titleBlock = `
      <section class="titlewrap">
        <div class="eyebrow"><span>Keynote</span></div>
        <h1 class="title" id="title">${TITLE}</h1>
      </section>`;

const meta = `<div class="meta">${DATE}<span class="dot">&middot;</span>${VENUE}</div>`;

const cta = `<a class="cta">Get tickets<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h13M12.5 6l6 6-6 6"/></svg></a>`;

const bodies = {
  square: `
    <div class="inner">
      ${brand}
      ${speaker}
      ${titleBlock}
      <div class="hr"></div>
      <div class="foot">${meta}${cta}</div>
    </div>`,

  landscape: `
    <div class="inner">
      <div class="left">
        ${brand}
        ${titleBlock}
        <div class="hr"></div>
        <div class="foot" style="margin-top:38px">${meta}${cta}</div>
      </div>
      <div class="right">
        <div class="avatar"><img src="portrait.png" alt="${NAME}"></div>
        <div class="who">
          <div class="name">${NAME}</div>
          <div class="role">${ROLE}</div>
        </div>
        <div class="badge">Keynote Speaker</div>
      </div>
    </div>`,

  story: `
    <div class="inner">
      ${brand}
      <div class="spacer"></div>
      ${speaker}
      <div class="spacer"></div>
      ${titleBlock}
      <div class="spacer"></div>
      <div class="hr"></div>
      <div class="foot" style="margin-top:46px">${meta}${cta}</div>
    </div>`,
};

// per-size CSS custom properties
const vars = {
  square: `
      --logo-w: 164px; --event-size: 21px; --rule-h: 34px; --brand-gap: 22px;
      --avatar: 150px; --ring: 4px; --sp-gap: 28px;
      --name-size: 41px; --role-size: 22px;
      --eyebrow-size: 16px; --eyebrow-gap: 20px;
      --title-size: 66px;
      --meta-size: 23px; --cta-size: 22px; --cta-pad: 19px 36px;`,
  landscape: `
      --logo-w: 180px; --event-size: 23px; --rule-h: 38px; --brand-gap: 24px;
      --avatar: 300px; --ring: 5px;
      --eyebrow-size: 17px; --eyebrow-gap: 24px;
      --title-size: 76px;
      --meta-size: 26px; --cta-size: 24px; --cta-pad: 21px 40px;`,
  story: `
      --logo-w: 172px; --event-size: 22px; --rule-h: 36px; --brand-gap: 22px;
      --avatar: 340px; --ring: 5px; --sp-gap: 34px;
      --name-size: 56px; --role-size: 28px; --role-gap: 12px;
      --eyebrow-size: 19px; --eyebrow-gap: 26px;
      --title-size: 76px;
      --meta-size: 29px; --cta-size: 28px; --cta-pad: 26px 52px;`,
};

const fit = `
<script>
// Shrink the talk title until it fits its container, so it never overflows.
function fitTitle() {
  var wrap = document.querySelector('.titlewrap');
  var el = document.getElementById('title');
  if (!wrap || !el) return;
  var avail = wrap.clientHeight
    - document.querySelector('.eyebrow').offsetHeight
    - parseFloat(getComputedStyle(document.querySelector('.eyebrow')).marginBottom)
    - parseFloat(getComputedStyle(wrap).paddingTop)
    - parseFloat(getComputedStyle(wrap).paddingBottom);
  var size = parseFloat(getComputedStyle(el).fontSize);
  var guard = 0;
  while (el.scrollHeight > avail + 0.5 && size > 14 && guard++ < 400) {
    size -= 1;
    el.style.fontSize = size + 'px';
  }
  el.dataset.fitted = size;
}
(document.fonts ? document.fonts.ready : Promise.resolve()).then(function () {
  return new Promise(function (r) { requestAnimationFrame(function () { requestAnimationFrame(r); }); });
}).then(function () {
  fitTitle();
  requestAnimationFrame(function () {
    requestAnimationFrame(function () { document.body.classList.add('ready'); });
  });
});
</script>`;

for (const size of ['square', 'landscape', 'story']) {
  const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${NAME} — ${EVENT}</title>
<link rel="stylesheet" href="card.css">
<style>body.${size} { ${vars[size]} }</style>
</head>
<body class="${size}">
  <div class="card">${bodies[size]}
  </div>
${fit}
</body>
</html>`;
  fs.writeFileSync(path.join(__dirname, size + '.html'), html);
  console.log('wrote', size + '.html');
}

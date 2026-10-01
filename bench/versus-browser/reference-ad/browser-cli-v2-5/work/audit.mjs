import { chromium } from '/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright/index.mjs';
const dir = process.cwd();
const cases = [['portrait',1080,1350],['wide',1200,1000],['sky',300,600]];
const b = await chromium.launch();
for (const [n,w,h] of cases) {
  const p = await b.newPage({viewport:{width:w,height:h}});
  await p.goto('file://'+dir+'/'+n+'.html');
  await p.waitForTimeout(300);
  const r = await p.evaluate(({w,h})=>{
    const out={doc:{sw:document.documentElement.scrollWidth,sh:document.documentElement.scrollHeight},items:[],gaps:[]};
    document.querySelectorAll('.headline,.nm,.rl,.cta span,.step span,.footer,.photoband,.cta,.cand').forEach(e=>{
      const b=e.getBoundingClientRect();
      out.items.push({c:e.className,t:(e.textContent||'').slice(0,22),x:+b.x.toFixed(1),y:+b.y.toFixed(1),w:+b.width.toFixed(1),h:+b.height.toFixed(1),
        ovX:e.scrollWidth-e.clientWidth, ovY:e.scrollHeight-e.clientHeight,
        oob: b.left<-0.5||b.top<-0.5||b.right>w+0.5||b.bottom>h+0.5});
    });
    const kids=[...document.body.children].map(e=>{const b=e.getBoundingClientRect();return {cls:e.className,top:+b.top.toFixed(1),bot:+b.bottom.toFixed(1)}});
    for(let i=1;i<kids.length;i++) out.gaps.push({between:kids[i-1].cls+'->'+kids[i].cls,gap:+(kids[i].top-kids[i-1].bot).toFixed(1)});
    out.kids=kids;
    const im=document.querySelector('.photoband img');
    out.img={nat:[im.naturalWidth,im.naturalHeight],box:[im.clientWidth,im.clientHeight]};
    return out;
  },{w,h});
  console.log('=== '+n+' '+w+'x'+h, JSON.stringify(r.doc), JSON.stringify(r.img));
  console.log(' gaps:', r.gaps.map(g=>g.between+'='+g.gap).join('  '));
  r.items.filter(i=>i.ovX>0||i.ovY>0||i.oob).forEach(i=>console.log('  !!',JSON.stringify(i)));
  await p.close();
}
await b.close();

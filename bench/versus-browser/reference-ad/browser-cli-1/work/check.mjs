import { chromium } from '/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/playwright/index.mjs';
const sizes=[['portrait',1080,1350],['wide',1200,1000],['sky',300,600]];
const b=await chromium.launch();
for(const [n,W,H] of sizes){
  const p=await b.newPage({viewport:{width:W,height:H}});
  await p.goto('file://'+process.cwd()+'/'+n+'.html');
  await p.evaluate(()=>document.fonts.ready);
  const r=await p.evaluate(({W,H})=>{
    const out={overflow:[],outside:[],doc:[document.documentElement.scrollWidth,document.documentElement.scrollHeight]};
    for(const el of document.querySelectorAll('*')){
      const b=el.getBoundingClientRect();
      if(el.scrollWidth>el.clientWidth+1||el.scrollHeight>el.clientHeight+1)
        out.overflow.push([el.className||el.tagName,el.scrollWidth,el.clientWidth,el.scrollHeight,el.clientHeight,(el.textContent||'').slice(0,24)]);
      if(b.width&&(b.left<-0.5||b.top<-0.5||b.right>W+0.5||b.bottom>H+0.5))
        out.outside.push([el.className||el.tagName,JSON.stringify([b.left,b.top,b.right,b.bottom].map(v=>+v.toFixed(1)))]);
    }
    // text vs card fit
    const g=(s)=>[...document.querySelectorAll(s)].map(e=>e.getBoundingClientRect());
    out.cards=g('.card').map(c=>+c.width.toFixed(1));
    out.names=[...document.querySelectorAll('.cname')].map(e=>{const r=document.createRange();r.selectNodeContents(e);return +r.getBoundingClientRect().width.toFixed(1)});
    out.cardInner=g('.card').map(c=>+c.width.toFixed(1)).map((w,i)=>w);
    out.gaps=g('.card').slice(1).map((c,i)=>+(c.left-g('.card')[i].right).toFixed(2));
    out.bands=[...document.querySelectorAll('.photo,.cta')].map(e=>{const r=e.getBoundingClientRect();return [e.className,+r.left.toFixed(1),+r.width.toFixed(1),+r.height.toFixed(1)]});
    out.steps=[...document.querySelectorAll('ol li span')].map(e=>{const r=document.createRange();r.selectNodeContents(e);const b=r.getBoundingClientRect();return [+b.width.toFixed(1),+b.height.toFixed(1),+e.getBoundingClientRect().width.toFixed(1)]});
    out.ctaText=(()=>{const e=document.querySelector('.cta span');const r=document.createRange();r.selectNodeContents(e);return +r.getBoundingClientRect().width.toFixed(1)})();
    out.sections=[...document.body.children].map(e=>{const r=e.getBoundingClientRect();return [e.tagName+'.'+e.className,+r.top.toFixed(1),+r.bottom.toFixed(1)]});
    return out;
  },{W,H});
  console.log('==',n,JSON.stringify(r,null,1));
  await p.close();
}
await b.close();

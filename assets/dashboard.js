// Read only visible usage controls, never chat messages or form fields.
(async () => {
 const visible = e => e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden';
 const nodes = [...document.querySelectorAll('[role=progressbar],progress')].filter(visible);
 const items=[];
 for(const e of nodes) {
   let root=e.parentElement;
   for(let i=0;i<3 && root && root.innerText.length<500;i++) {
     if(/usage|limit|wykorzyst|zuzy|zuży|remaining|pozosta/i.test(root.innerText)) break;
     root=root.parentElement;
   }
   const context=(root?.innerText||e.getAttribute('aria-label')||'').trim();
   if(!/usage|limit|wykorzyst|zuzy|zuży|remaining|pozosta/i.test(context)) continue;
   const raw=e.getAttribute('aria-valuenow') ?? (e.tagName==='PROGRESS' ? e.value : null);
   const max=Number(e.getAttribute('aria-valuemax') ?? (e.tagName==='PROGRESS'?e.max:100));
   const min=Number(e.getAttribute('aria-valuemin')||0);
   if(raw===null || !Number.isFinite(max)||max<=min) continue;
   let p=(Number(raw)-min)/(max-min)*100;
   const remaining=/remaining|pozosta/i.test((e.getAttribute('aria-label')||'')+' '+(e.getAttribute('aria-valuetext')||''));
   if(remaining)p=100-p;
   if(!Number.isFinite(p)||p<0||p>100)continue;
   const label=e.getAttribute('aria-label') || context.split('\n').filter(t=>!t.includes('%')).slice(0,2).join(' / ');
   if(label)items.push({label:label.slice(0,100),percent:p});
 }
 // Text-only counters are accepted only with explicit used/remaining semantics
 // inside a usage dialog/section, never from the chat transcript.
 const roots=[...document.querySelectorAll('[role=dialog],[aria-label*=usage i],[aria-label*=limit i],[data-testid*=usage i]')].filter(visible);
 for(const root of roots) {
   if(!/usage|limit|wykorzyst/i.test(root.innerText)) continue;
   for(const e of [...root.querySelectorAll('span,p')].filter(visible)) {
     const t=e.innerText.trim(); if(t.length>140)continue;
     const m=t.match(/^(\d+(?:[.,]\d+)?)\s*%\s*(used|consumed|remaining|wykorzystan\w*|zuży\w*|pozosta\w*)/i);
     if(!m)continue; let p=Number(m[1].replace(',','.'));if(/remaining|pozosta/i.test(m[2]))p=100-p;
     if(p<0||p>100)continue;
     const heading=e.parentElement?.querySelector('h1,h2,h3,h4,[role=heading]')?.innerText;
     if(heading)items.push({label:heading.trim().slice(0,100),percent:p});
   }
 }
 const unique=items.filter((e,i,a)=>a.findIndex(x=>x.label===e.label&&x.percent===e.percent)===i);
 return {items:unique};
})()

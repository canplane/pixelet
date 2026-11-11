// node measure.js LABEL : Safari's page memory by phase. The page on VPORT behind mem-proxy.js, whose script beacons its
// live GPU bytes to the collector here (:8099). Every 5 s: the page's live bytes and object count, the GPU process's
// footprint and RSS, the GPU's utilization, into LABEL.jsonl. Phases: load, default zoom, farthest zoom with the sky time
// moving, back to default, shadows on. Stops at once if the GPU pins at 100% or the footprint passes 1.2 GB. The window is
// closed and the session ended on every way out, the handlers set before it is opened.
import fs from 'node:fs';import http from 'node:http';import {execSync} from 'node:child_process';
const L=process.argv[2],VPORT=process.env.VPORT||8093,O=`http://localhost:${VPORT}`,WD='http://localhost:4444';
const SEC=k=>Number(process.env[k]);const DEFAULT=SEC('DEFAULT_S')||120,FAR=SEC('FAR_S')||300,BACK=SEC('BACK_S')||60,SHADOW=SEC('SHADOW_S')||60;
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const out=`${L}.jsonl`;const rec=o=>fs.appendFileSync(out,JSON.stringify({t:Date.now(),...o})+'\n');
let S=null,phase='start',page={};
const srv=http.createServer((q,r)=>{let b='';q.on('data',c=>b+=c);q.on('end',()=>{r.setHeader('access-control-allow-origin','*');r.end('ok');try{const v=JSON.parse(b);if(q.url==='/m')page=v;else rec({phase,beacon:q.url,v})}catch{}})}).listen(8099);
const endNow=async c=>{rec({phase,end:c});if(S){for(const p of [S+'/window',S]){try{await fetch(WD+p,{method:'DELETE',signal:AbortSignal.timeout(8000)})}catch(e){console.log('delete',String(e))}}}process.exit(c)};
for(const g of ['SIGALRM','SIGTERM','SIGINT'])process.on(g,()=>endNow(9));
setTimeout(()=>{console.log('deadline');endNow(8)},(DEFAULT+FAR+BACK+SHADOW+170)*1000).unref();
process.on('uncaughtException',e=>{console.log('fail',String(e));endNow(7)});
process.on('unhandledRejection',e=>{console.log('fail',String(e));endNow(7)});
const gpuPids=()=>{try{return execSync("ps -axo pid=,comm= | grep -i 'WebKit.GPU' | grep -v grep | awk '{print $1}'").toString().trim().split('\n').filter(Boolean)}catch{return[]}};
const footprint=()=>{let total=0,iog=0;const each=[];for(const p of gpuPids()){try{const t=execSync(`footprint ${p}`,{timeout:4000}).toString();const m=t.match(/Footprint:\s*([\d.]+)\s*(KB|MB|GB)/);const f=m?Number(m[1])*{KB:1/1024,MB:1,GB:1024}[m[2]]:0;const io=t.split('\n').filter(l=>/IOAccelerator|IOSurface|Metal/i.test(l)).map(l=>l.trim().replace(/\s+/g,' '));total+=f;each.push({p,f:Math.round(f)});iog=io.slice(0,4)}catch{}}return{mb:Math.round(total),each,iog}};
const util=()=>{try{return Number((execSync('ioreg -r -d 1 -w 0 -c IOAccelerator').toString().match(/"Device Utilization %"=(\d+)/)||[])[1])}catch{return -1}};
let pinned=0;
const sample=()=>{const fp=footprint(),u=util();rec({phase,live:page.live,peak:page.peak,n:page.n,made:page.made,gone:page.gone,inflight:page.inflight,fp:fp.mb,each:fp.each,iog:fp.iog,util:u});console.log(phase,'live',Math.round((page.live||0)/1048576),'n',page.n,'fp',fp.mb,'util',u);if(u>=100)pinned++;else pinned=0;if(pinned>=3){console.log('STOP pinned');endNow(5)}if(fp.mb>1200){console.log('STOP footprint');endNow(6)}};
sample();const iv=setInterval(sample,5000);
const wd=async(m,p,b,ms=60000)=>{const r=await fetch(WD+p,{method:m,headers:{'content-type':'application/json'},body:b&&JSON.stringify(b),signal:AbortSignal.timeout(ms)});const j=await r.json();if(j.value&&j.value.error)throw new Error(j.value.error+': '+j.value.message);return j.value};
phase='session';const s=await wd('POST','/session',{capabilities:{alwaysMatch:{browserName:'safari'}}},150000);S=`/session/${s.sessionId}`;
const ev=b=>wd('POST',`${S}/execute/async`,{script:`const done=arguments[arguments.length-1];Promise.resolve().then(async()=>(${b})).then(v=>done({ok:v}),e=>done({err:String(e)}))`,args:[]},30000).then(v=>{if(v&&'err' in v)throw new Error(v.err);return v&&v.ok});
const top=async()=>{try{rec({phase,top:await ev(`__top()`),errs:await ev(`(window.__E||[]).slice(0,10)`)})}catch(e){rec({phase,top_err:String(e)})}};
await wd('POST',`${S}/timeouts`,{script:30000});
await wd('POST',`${S}/window/rect`,{x:0,y:0,width:540,height:1100});
phase='load';await wd('POST',`${S}/url`,{url:O+'/'});
let ok=false;for(let i=0;i<240;i++){await wait(500);try{const st=await ev(`({p:typeof pxl,c:!!document.querySelector('canvas'),l:document.body.innerText.includes('공간을 불러오고')||document.body.innerText.includes('Loading')})`);if(st.p==='object'&&st.c&&!st.l){ok=true;break}}catch{}}
console.log('shown',ok);rec({phase,shown:ok});if(!ok)await endNow(2);
await ev(`(()=>{const ms=__r.getModules();const ent=ms instanceof Map?[...ms]:Object.entries(ms);const get=p=>{const e=ent.find(([,m])=>m&&m.verboseName===p);return __r(Number(e[0]))};window.__S=get('src/port/setting.ts');window.__Q=get('src/abi/schema.spec.ts');__S.set(__Q.SETTING.res.key,__Q.SETTING.res.options.fixed);__S.set(__Q.SETTING.shadows.key,0);return 1})()`);
const input=`(async(wheels,tilt)=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();const [x,y]=[r.left+r.width/2,r.top+r.height/2];const w=d=>cv.dispatchEvent(new WheelEvent('wheel',{deltaY:d,clientX:x,clientY:y,bubbles:true,cancelable:true}));const p=(t,yy,b)=>cv.dispatchEvent(new PointerEvent(t,{pointerId:1,pointerType:'mouse',isPrimary:true,clientX:x,clientY:yy,buttons:b,button:2,bubbles:true,cancelable:true}));for(let n=0;n<Math.abs(wheels);n++){w(Math.sign(wheels)*100);await new Promise(r=>setTimeout(r,16))}if(tilt){p('pointerdown',y,2);for(let n=1;n<=60;n++){p('pointermove',y+tilt*n/60,2);await new Promise(r=>setTimeout(r,16))}p('pointerup',y+tilt,0)}return 1})`;
phase='default';await wait(DEFAULT*1000);await top();
phase='far';await ev(input+`(60,-2000)`);
const times=['noon','afternoon','dusk','dawn'];const t0=Date.now();let k=0;
while(Date.now()-t0<FAR*1000){await ev(`(__S.set(__Q.SETTING.sky_time.key,__Q.SETTING.sky_time.options.${times[k%4]}),1)`);k++;await wait(20000)}
await top();
phase='back';await ev(input+`(-60,2000)`);await wait(BACK*1000);await top();
phase='shadows';await ev(`(__S.set(__Q.SETTING.shadows.key,1),1)`);await wait(SHADOW*1000);await top();
clearInterval(iv);console.log('done');await endNow(0);

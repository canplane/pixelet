// node pyramid.js s23|mac <label>: the real page's observed pyramid, read back as it runs, each kept entry
// against what the code says it must be: its slot in the bucket its zoom and node hash to, a zoom a walk asks, a kind.
import {client,ev} from './cdp.js';
import {execSync,spawn} from 'node:child_process';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';
const D=process.argv[2],LABEL=process.argv[3]||'run';const PORT=D==='s23'?9333:9222;
const O='http://localhost:5174';const W=process.cwd()+'/wt/ts/client/src';
const A=process.env.HOME+'/Library/Android/sdk/platform-tools/adb';
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const file=`pyramid-${D}-${LABEL}.json`;const out={device:D,label:LABEL,log:[]};const log=(k,v)=>{out.log.push([k,v]);console.log(k,JSON.stringify(v).slice(0,230));fs.writeFileSync(file,JSON.stringify(out,null,1))};
const inject=`(()=>{
 const E=${1<<19},EW=12,MARKS=${1<<20};
 const P={buf:null,dev:null,copy:null,skipped:0};window.__P=P;
 P.cands=[];const cb=GPUDevice.prototype.createBuffer;
 GPUDevice.prototype.createBuffer=function(d){const h=d.size/4-(E*EW+1+MARKS);if(h>0&&h<5000&&Number.isInteger(h)&&!(d.usage&GPUBufferUsage.MAP_READ)){const b=cb.call(this,{...d,usage:d.usage|GPUBufferUsage.COPY_SRC});P.buf=b;P.dev=this;P.head=h;P.size=d.size;return b}if(d.size>8000000&&(d.usage&GPUBufferUsage.STORAGE)&&!(d.usage&GPUBufferUsage.MAP_READ)){const b=cb.call(this,{...d,usage:d.usage|GPUBufferUsage.COPY_SRC});P.cands.push(b);P.dev=this;return b}return cb.call(this,d)};

 const DBG=2000000;
 P.stage=[];const sub=GPUQueue.prototype.submit;let tick=false;const raf=()=>{tick=true;requestAnimationFrame(raf)};requestAnimationFrame(raf);
 GPUQueue.prototype.submit=function(c){const r=sub.call(this,c);if(P.want>0&&tick&&P.buf&&P.beams){tick=false;const d=P.dev;const i=2-P.want;P.want--;if(!P.stage[i])P.stage[i]=[cb.call(d,{size:P.size,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST}),cb.call(d,{size:DBG*4,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST})];const e=d.createCommandEncoder();e.copyBufferToBuffer(P.buf,0,P.stage[i][0],0,P.size);e.copyBufferToBuffer(P.beams,P.beams.size-DBG*4,P.stage[i][1],0,DBG*4);sub.call(this,[e.finish()]);if(P.want===0&&P.done)P.done()}return r};
 P.find=async()=>{const d=P.dev;for(const b of P.cands.slice().reverse()){if(b.size<DBG*4)continue;const s=cb.call(d,{size:4,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST});const e=d.createCommandEncoder();e.copyBufferToBuffer(b,b.size-4,s,0,4);d.queue.submit([e.finish()]);await s.mapAsync(GPUMapMode.READ);const v=new Uint32Array(s.getMappedRange())[0];s.unmap();s.destroy();if(v===0xDB6DB6DB){P.beams=b;return b.size}}return 0};
 P.pair=async()=>{if(!P.beams){const f=await P.find();if(!f)return {err:'no beam buffer yet',cands:P.cands.map(b=>b.size)}}
  P.want=2;await new Promise(r=>P.done=r);P.done=null;
  await Promise.all(P.stage.flat().map(b=>b.mapAsync(GPUMapMode.READ)));
  const py=P.stage.map(s=>new Uint32Array(s[0].getMappedRange())),db=P.stage.map(s=>new Uint32Array(s[1].getMappedRange()));
  const f=py.map(u=>u[0]);const at=P.head;
  const held=new Set();for(let i=0;i<E;i++){const w=at+i*EW,k=py[0][w];if(k===0||k===0xFFFFFFFF)continue;held.add(py[0][w+8]+','+py[0][w+9]+','+py[0][w+10]+','+py[0][w+11])}
  const zones=[['beam',0,20000],['sample',20000,120000],['miss',120000,333000]];const out={frames:f,kept0:held.size};
  for(const [n,a,b] of zones){const seen=new Set();const reason=[0,0,0,0,0,0,0,0,0];const deltas={};let rays=0,absentPrev=0,zooms={};const u=db[1];for(let i=a;i<b;i++){const k=i*6;if((u[k]&0xFFFFFF)!==((f[1]+1)&0xFFFFFF))continue;rays++;const r=u[k+5]&255;reason[r]=(reason[r]||0)+1;if(r===3||r===7||r===8){const af=(u[k+5]>>>8)&255,an=(u[k+5]>>>16)&255,hf=(u[k+5]>>>24)&255,hn=(u[k]>>>24)&255;const key=(hf-af)+','+(hn-an);deltas[key]=(deltas[key]||0)+1}const id=u[k+1]+','+u[k+2]+','+u[k+3]+','+u[k+4];if(r!==6)seen.add(id);zooms[u[k+1]]=(zooms[u[k+1]]||0)+1;if((r===2||r===4)&&held.has(id))absentPrev++}out[n]={rays,distinct:seen.size,reasons:{keeps:reason[1],absent_free:reason[2],far_only:reason[3],near_only:reason[7],both:reason[8],absent_full:reason[4],claimed:reason[5],steps:reason[6]},absent_held_last_frame:absentPrev,zooms,deltas}}
  P.stage.flat().forEach(b=>b.unmap());return out};

 P.depth=async()=>{const d=P.dev;if(!P.st)P.st=cb.call(d,{size:P.size,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST});await new Promise(r=>requestAnimationFrame(r));await new Promise(r=>setTimeout(r,0));const e=d.createCommandEncoder();e.copyBufferToBuffer(P.buf,0,P.st,0,P.size);d.queue.submit([e.finish()]);await P.st.mapAsync(GPUMapMode.READ);const u=new Uint32Array(P.st.getMappedRange());const fr=u[0];const at=P.head;const passes=[new Map(),new Map(),new Map()];let others=0;
  for(let i=0;i<E;i++){const w=at+i*EW,k=u[w];if(k===0||k===0xFFFFFFFF)continue;const tag=k&((1<<20)-1);if(Math.floor(tag/6)!==fr%174762){others++;continue}const p=tag%6;if(p<3)passes[p].set(u[w+8]+','+(u[w+9]|0)+','+(u[w+10]|0)+','+(u[w+11]|0),u[w+8])}
  const out={frame:fr,older:others};['beam','sample','miss'].forEach((n,p)=>{const m=passes[p];const hist={};const dep=new Map();const keys=[...m.entries()].sort((a,b)=>a[1]-b[1]);let most=0;for(const [key,z] of keys){const [zz,x,y,c]=key.split(',').map(Number);const par=(z-1)+','+(x>>1)+','+(y>>1)+','+(c>>1);const dp=(dep.get(par)||0)+1;dep.set(key,dp);hist[dp]=(hist[dp]||0)+1;if(dp>most)most=dp}out[n]={nodes:m.size,deepest:most,by_round:hist}});P.st.unmap();return out};
 const mix=(h,v)=>{const m=Math.imul(h^v,0x9E3779B1)>>>0;return ((m<<15)|(m>>>17))>>>0};
 const entryOf=(z,x,y,w)=>{let h=Math.imul(z,0x85EBCA77)>>>0;h=mix(h,x);h=mix(h,y);h=mix(h,w);h=(h^(h>>>16))>>>0;return (h&(E-1)&~3)>>>0};
 const keeps=(z,x,y,w)=>{if(z>=32)return false;const a=(x|0)>>z,b=(y|0)>>z,c=(w|0)>>z;return a===0&&(b===0||b===-1)&&(c===0||c===-1)};
 P.snap=async()=>{const d=P.dev,b=P.buf;if(!b)return {err:'no buffer'};if(!P.copy)P.copy=d.createBuffer({size:P.size,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST});
  const e=d.createCommandEncoder();e.copyBufferToBuffer(b,0,P.copy,0,P.size);d.queue.submit([e.finish()]);await P.copy.mapAsync(GPUMapMode.READ);const u=new Uint32Array(P.copy.getMappedRange());
  let kept=0,claimed=0,zero=0,badZoom=0,badKeep=0,badBucket=0,badKind=0,samples=[];const at=P.head;
  for(let i=0;i<E;i++){const w=at+i*EW,key=u[w];if(key===0){zero++;continue}if(key===0xFFFFFFFF){claimed++;continue}kept++;
   const kind=u[w+1],z=u[w+8],x=u[w+9],y=u[w+10],c=u[w+11];let bad=null;
   if(z>=33){badZoom++;bad='zoom'}else if(!keeps(z,x,y,c)){badKeep++;bad='keep'}else if(entryOf(z,x,y,c)!==(i&~3)>>>0){badBucket++;bad='bucket'}else if(kind>2){badKind++;bad='kind'}
   if(bad&&samples.length<6)samples.push([i,bad,key>>>0,kind,z,x,y,c])}
  let dump=null;if(P.want){dump=[];for(let i=0;i<E;i++){const w=at+i*EW,key=u[w];if(key===0||key===0xFFFFFFFF)continue;dump.push(key>>>20,u[w+8],u[w+9],u[w+10],u[w+11],u[w+1],u[w+2],u[w+3],u[w+4],u[w+5],u[w+6],u[w+7])}P.want=false}
  const frame=u[0];P.copy.unmap();return {frame,kept,claimed,zero,badZoom,badKeep,badBucket,badKind,samples,dump}};
})();`;
const therm=()=>D!=='s23'?'mac':execSync(`${A} shell dumpsys thermalservice`).toString().split('\n').filter(l=>/Thermal Status|mName=AP\b/.test(l)).slice(0,2).map(s=>s.trim().replace(/Temperature\{mValue=|, mType=\d+|mName=|\}|, mStatus=\d/g,'')).join(' | ');
let chrome,dir;
if(D==='s23'){execSync(`${A} reverse tcp:5174 tcp:5174`);execSync(`${A} forward tcp:9333 localabstract:chrome_devtools_remote`);execSync(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d about:blank`)}
else{dir=fs.mkdtempSync(path.join(os.tmpdir(),'pxl-pyr-'));chrome=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[`--user-data-dir=${dir}`,'--lang=ko','--remote-debugging-port=9222','--no-first-run','--no-default-browser-check','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding','--disable-background-timer-throttling','about:blank'],{stdio:'ignore'})}
let t;for(let i=0;i<30&&!t;i++){await wait(1000);try{const tabs=await (await fetch(`http://localhost:${PORT}/json`)).json();t=tabs.find(x=>x.type==='page'&&x.url==='about:blank')??tabs.find(x=>x.type==='page'&&x.url.startsWith(O))}catch{}}
if(!t)throw new Error('no tab');
const v=await (await fetch(`http://localhost:${PORT}/json/version`)).json();const b=client(v.webSocketDebuggerUrl);await b.open;await b.call('Browser.grantPermissions',{origin:O,permissions:['geolocation']});
const c=client(t.webSocketDebuggerUrl);await c.open;await c.call('Page.enable');await c.call('Runtime.enable');
if(D==='mac'){const dpr=await ev(c,'devicePixelRatio',5000);await c.call('Emulation.setDeviceMetricsOverride',{width:Math.round(1080/dpr),height:Math.round(1968/dpr),deviceScaleFactor:0,mobile:false})}
await c.call('Emulation.setGeolocationOverride',{latitude:38.1193,longitude:128.4656,accuracy:10});
await c.call('Page.addScriptToEvaluateOnNewDocument',{source:inject});
log('clear',(await c.call('Storage.clearDataForOrigin',{origin:O,storageTypes:'all'})).error||'ok');await c.call('Page.bringToFront');await c.call('Page.navigate',{url:O+'/'});await wait(1500);await c.call('Page.bringToFront');
const t0=Date.now();const T=()=>Math.round((Date.now()-t0)/1000);
await ev(c,`navigator.wakeLock.request('screen').then(l=>{window.__wl0=l;return 1}).catch(e=>String(e))`,5000).then(x=>log('wake',x));
const P='/@fs'+W;let up=false;
const geo=()=>c.call('Emulation.setGeolocationOverride',{latitude:38.1193,longitude:128.4656,accuracy:10});await geo();log('geo test',await ev(c,`new Promise(res=>navigator.geolocation.getCurrentPosition(p=>res(['ok',p.coords.latitude]),e=>res(['err',e.code,e.message]),{timeout:4000}))`,8000));
for(let i=0;i<Number(process.env.WAIT||240);i++){await wait(1000);if(i%5===0)await geo();try{if(!up){const s=await ev(c,`typeof pxl`,4000);if(s==='object'){await ev(c,`(async()=>{const S=await import('${P}/port/setting.ts');const Q=await import('${P}/shell/setting/schema.ts');const F=await import('${P}/port/perf.ts');window.__m={set:S.kernel_setting,SET:Q.SETTING,start:F.kernel_perf_start};__m.set(__m.SET.perf.key,1);__m.set(__m.SET.res.key,__m.SET.res.options.fixed);__m.set(__m.SET.shadows.key,0);__m.start();return 1})()`);up=true}}else{const r=await ev(c,`(()=>{const v=(pxl.perf.report().match(/view +pivot ([^ ]+)/)||[])[1];const f=pxl.perf.read().frames||{};return {read:f.gpu_read,pivot:v||'-',has:!!__P.buf}})()`,4000);if(r.read>0&&r.pivot!=='-'){log('live at s',{s:T(),...r});break}if(i%30===0)log('wait '+T(),r)}}catch(e){}}
if(out.log.every(([k])=>k!=='live at s')){log('invalid','the world never drew');b.close();c.close();chrome?.kill();process.exit(2)}
const dumps=[];const snap=async k=>{await ev(c,`__P.want=true`);const r=await ev(c,`__P.snap()`,120000);dumps.push(r.dump);delete r.dump;log(k+' @'+T(),r);fs.writeFileSync(`entries-${D}-${LABEL}.json`,JSON.stringify(dumps))};
const motion=(secs,turn)=>`(async()=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();const [x,y]=[r.left+r.width/2,r.top+r.height/2];const p=(t,a,b)=>cv.dispatchEvent(new PointerEvent(t,{pointerId:1,pointerType:'mouse',isPrimary:true,clientX:a[0],clientY:a[1],buttons:b,button:2,bubbles:true,cancelable:true}));const on=a=>[x+100*Math.cos(a),y+100*Math.sin(a)];p('pointerdown',on(-Math.PI/2),2);const t0=performance.now();const s=setInterval(()=>p('pointermove',on(-Math.PI/2+(performance.now()-t0)/1000/${turn}*2*Math.PI),2),16);await new Promise(r=>setTimeout(r,${secs}*1000));clearInterval(s);p('pointerup',on(-Math.PI/2),0);return 1})()`;
if(process.env.STILL){if(process.env.SHOT){await ev(c,`__m.set(__m.SET.sky_time.key,__m.SET.sky_time.options.dawn)`)}await wait(Number(process.env.STILL)*1000);await snap('still');
 if(process.env.SHOT){for(const n of [1,2]){const r=await c.call('Page.captureScreenshot',{format:'png'});if(r.error)log('shot error',r.error.message);else{fs.writeFileSync(`shot-${D}-${LABEL}-${n}.png`,Buffer.from(r.result.data,'base64'));log('shot',n)}await wait(4000)}}
await wait(10000);await snap('still again');log('done',T());b.close();if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}process.exit(0)}
await wait(10000);
log('camera after world',await ev(c,`(pxl.perf.report().match(/distance [0-9.]+ u +polar [-0-9.]+. +azimuth [-0-9.]+/)||[''])[0]`));const pairs=async k=>{const r=await ev(c,`__P.pair()`,60000);log(k,r)};
await ev(c,`__m.set(__m.SET.res.key,__m.SET.res.options.fixed);__m.set(__m.SET.ray_counts.key,0)`);
if(process.env.MODE==='frames'){await ev(c,`(async()=>{const F=await import('${P}/port/perf.ts');window.__m.frames=F.perf_frames;__m.set(__m.SET.ray_counts.key,1);__m.start();return 1})()`);await wait(1500);await ev(c,motion(Number(process.env.SECS||40),6),240000);
 const rows=await ev(c,`(()=>{const names=(pxl.perf.read().frames.gpu_spans||[]).map(x=>x.name);const pick=['beam','sample_hit','sample','miss','face','generate','world'].map(n=>names.indexOf(n));return JSON.stringify({names,rows:__m.frames().map(f=>[f.t,f.interval_ms,f.gpu_read,f.gpu_busy_ms,f.gpu_frame,f.eye_cast,f.eye_beam,f.eye_sample,f.eye_miss,f.settled,...pick.map(i=>f.gpu_spans[i])])})})()`,60000);fs.writeFileSync(`frames-${D}-${LABEL}.json`,rows);log('frames',JSON.parse(rows).rows.length)}
if(process.env.MODE==='depth'){await ev(c,`__m.set(__m.SET.ray_counts.key,0)`);const circle2=ev(c,motion(Number(process.env.SECS||50),6),240000);await wait(4000);for(let k=0;k<Number(process.env.PAIRS||12);k++){log('depth '+k,await ev(c,`__P.depth()`,60000));await wait(1500)}await circle2}
if(process.env.MODE){log('done',T());b.close();if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}process.exit(0)}
const circle=ev(c,motion(Number(process.env.SECS||50),Number(process.env.TURN||6)),240000);
await wait(5000);for(let k=0;k<Number(process.env.PAIRS||10);k++){await pairs('pair '+k);await wait(2500)}
await circle;log('done',T());b.close();if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}process.exit(0);
for(let k=0;k<4;k++){await ev(c,motion(12,6),120000);await snap('circle '+k)}
log('therm',therm());
await ev(c,`__m.start()`);await ev(c,`pxl.perf.drive(15).then(()=>1)`,240000);await snap('drive');await ev(c,motion(12,4),120000);await snap('drive turn');
const line=await ev(c,`pxl.perf.report().split('\\n').filter(l=>/frames n=|kinds|gpu   /.test(l)).map(l=>l.slice(0,110)).join(' | ')`);log('report',line);log('therm',therm());
log('done',T());b.close();
if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();for(const x of (await (await fetch(`http://localhost:${PORT}/json`)).json()).filter(x=>x.type==='page'&&x.url==='about:blank'&&x.id!==t.id)){await fetch(`http://localhost:${PORT}/json/close/${x.id}`)}execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}
process.exit(0);

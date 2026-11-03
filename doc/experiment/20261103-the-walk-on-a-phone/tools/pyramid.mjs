// node pyramid.mjs s23|mac <label>: the real page's observed pyramid, read back as it runs, each kept entry
// against what the code says it must be: its slot in the bucket its zoom and node hash to, a zoom a walk asks, a kind.
import {client,ev} from './cdp.mjs';
import {execSync,spawn} from 'node:child_process';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';
const D=process.argv[2],LABEL=process.argv[3]||'run';const PORT=D==='s23'?9333:9222;
const O='http://localhost:5174';const W=process.cwd()+'/wt/ts/client/src';
const A=process.env.HOME+'/Library/Android/sdk/platform-tools/adb';
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const file=`pyramid-${D}-${LABEL}.json`;const out={device:D,label:LABEL,log:[]};const log=(k,v)=>{out.log.push([k,v]);console.log(k,JSON.stringify(v).slice(0,230));fs.writeFileSync(file,JSON.stringify(out,null,1))};
const inject=`(()=>{
 const E=${1<<19},EW=12,MARKS=${1<<20};
 const P={buf:null,dev:null,copy:null,skipped:0};window.__P=P;
 const cb=GPUDevice.prototype.createBuffer;
 GPUDevice.prototype.createBuffer=function(d){const h=d.size/4-(E*EW+1+MARKS);if(h>0&&h<5000&&Number.isInteger(h)&&!(d.usage&GPUBufferUsage.MAP_READ)){const b=cb.call(this,{...d,usage:d.usage|GPUBufferUsage.COPY_SRC});P.buf=b;P.dev=this;P.head=h;P.size=d.size;return b}return cb.call(this,d)};
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
 if(process.env.SHOT){log('view',await ev(c,`(pxl.perf.report().match(/distance [0-9.]+ u +polar [-0-9.]+. +azimuth [-0-9.]+/)||[''])[0]`));for(const n of [1,2]){const r=await c.call('Page.captureScreenshot',{format:'png'});if(r.error)log('shot error',r.error.message);else{fs.writeFileSync(`shot-${D}-${LABEL}-${n}.png`,Buffer.from(r.result.data,'base64'));log('shot',n)}await wait(4000)}
  for(const [tag,steps] of [['w',8],['x',8]]){await ev(c,`(async()=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();for(let n=0;n<${steps};n++){cv.dispatchEvent(new WheelEvent('wheel',{deltaY:100,clientX:r.left+r.width/2,clientY:r.top+r.height/2,bubbles:true,cancelable:true}));await new Promise(s=>setTimeout(s,30))}return 1})()`,30000);await wait(25000);const v=await ev(c,`(pxl.perf.report().match(/distance [0-9.]+ u +polar [-0-9.]+. +azimuth [-0-9.]+/)||[''])[0]`);const r2=await c.call('Page.captureScreenshot',{format:'png'});if(!r2.error){fs.writeFileSync(`shot-${D}-${LABEL}-${tag}.png`,Buffer.from(r2.result.data,'base64'));log('shot '+tag,v)}}
 }
await wait(10000);await snap('still again');log('done',T());b.close();if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}process.exit(0)}
await wait(10000);await snap('settled');
for(let k=0;k<4;k++){await ev(c,motion(12,6),120000);await snap('circle '+k)}
log('therm',therm());
await ev(c,`__m.start()`);await ev(c,`pxl.perf.drive(15).then(()=>1)`,240000);await snap('drive');await ev(c,motion(12,4),120000);await snap('drive turn');
const line=await ev(c,`pxl.perf.report().split('\\n').filter(l=>/frames n=|kinds|gpu   /.test(l)).map(l=>l.slice(0,110)).join(' | ')`);log('report',line);log('therm',therm());
log('done',T());b.close();
if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();for(const x of (await (await fetch(`http://localhost:${PORT}/json`)).json()).filter(x=>x.type==='page'&&x.url==='about:blank'&&x.id!==t.id)){await fetch(`http://localhost:${PORT}/json/close/${x.id}`)}execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}
process.exit(0);

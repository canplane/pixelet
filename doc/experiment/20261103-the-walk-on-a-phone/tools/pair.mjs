// node run4.mjs s23|mac : one device's page on the worktree, the same camera path and CSS viewport.
import {client,ev} from './cdp.mjs';
import {execSync,spawn} from 'node:child_process';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';
const D=process.argv[2];const PORT=D==='s23'?9333:9222;
const O=`http://localhost:${process.env.VPORT||5174}`;const W=process.cwd()+'/'+(process.env.WT||'wt')+'/ts/client/src';
const A=process.env.HOME+'/Library/Android/sdk/platform-tools/adb';
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const out={device:D,log:[]};const file=`pair-${process.env.LABEL}.json`;const log=(k,v)=>{out.log.push([k,v]);console.log(k,typeof v==='string'?v.slice(0,160):JSON.stringify(v).slice(0,160));fs.writeFileSync(file,JSON.stringify(out,null,1))};
const therm=()=>D!=='s23'?'mac':execSync(`${A} shell dumpsys thermalservice`).toString().split('\n').filter(l=>/Thermal Status|mName=(AP|SKIN)\b/.test(l)).slice(0,3).map(s=>s.trim().replace(/Temperature\{mValue=|, mType=\d+|mName=|\}|, mStatus=\d/g,'')).join(' | ');
let chrome,dir;
if(D==='s23'){execSync(`${A} reverse tcp:${process.env.VPORT||5174} tcp:${process.env.VPORT||5174}`);execSync(`${A} forward tcp:9333 localabstract:chrome_devtools_remote`);execSync(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d about:blank`)}
else{dir=fs.mkdtempSync(path.join(os.tmpdir(),'pxl-mac-'));chrome=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[`--user-data-dir=${dir}`,'--remote-debugging-port=9222','--lang=ko','--no-first-run','--no-default-browser-check','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding','--disable-background-timer-throttling','about:blank'],{stdio:'ignore'})}
let tabs;for(let i=0;i<30;i++){await wait(1000);try{tabs=await (await fetch(`http://localhost:${PORT}/json`)).json();if(tabs.some(x=>x.type==='page'))break}catch{}}
const t=D==='s23'?(tabs.find(x=>x.type==='page'&&x.url==='about:blank')??tabs.find(x=>x.type==='page'&&x.url.startsWith(O))):tabs.find(x=>x.type==='page');
if(!t)throw new Error('no blank tab');
if(D==='s23'){for(const x of (await (await fetch(`http://localhost:${PORT}/json`)).json()).filter(x=>x.type==='page'&&x.id!==t.id&&(x.url.startsWith('http://localhost')||x.url==='about:blank')))await fetch(`http://localhost:${PORT}/json/close/${x.id}`);await fetch(`http://localhost:${PORT}/json/activate/${t.id}`)}
const v=await (await fetch(`http://localhost:${PORT}/json/version`)).json();const b=client(v.webSocketDebuggerUrl);await b.open;const gr=await b.call('Browser.grantPermissions',{origin:O,permissions:['geolocation']});log('grant',gr.error||'ok');
const c=client(t.webSocketDebuggerUrl);await c.open;const GEO=`(()=>{const pos=()=>({coords:{latitude:38.1193,longitude:128.4656,accuracy:10,altitude:null,altitudeAccuracy:null,heading:null,speed:null},timestamp:Date.now()});const g=navigator.geolocation;let id=1;g.getCurrentPosition=(ok)=>{setTimeout(()=>ok(pos()),0)};g.watchPosition=(ok)=>{setTimeout(()=>ok(pos()),0);return id++};g.clearWatch=()=>{}})();`;await c.call('Page.enable');const ERR=`(()=>{window.__E=[];const push=(k,a)=>{if(window.__E.length<60)window.__E.push(k+': '+a.map(x=>String(x&&x.message||x)).join(' ').slice(0,400))};for(const k of ['error','warn']){const f=console[k];console[k]=(...a)=>{push(k,a);f.apply(console,a)}};addEventListener('error',e=>push('page',[e.message]));addEventListener('unhandledrejection',e=>push('rejection',[e.reason]));const rd=GPUAdapter.prototype.requestDevice;GPUAdapter.prototype.requestDevice=function(...a){return rd.apply(this,a).then(d=>{d.addEventListener('uncapturederror',e=>push('gpu',[e.error]));d.lost.then(i=>push('lost',[i.reason,i.message]));return d})}})();`;
await c.call('Page.addScriptToEvaluateOnNewDocument',{source:ERR+GEO});await c.call('Input.setIgnoreInputEvents',{ignore:true});
if(D==='mac'){const dpr=await ev(c,'devicePixelRatio',5000);await c.call('Emulation.setDeviceMetricsOverride',{width:Math.round(1080/dpr),height:Math.round(1968/dpr),deviceScaleFactor:0,mobile:false});log('mac dpr',dpr)}
await c.call('Emulation.setGeolocationOverride',{latitude:38.1193,longitude:128.4656,accuracy:10});
log('clear',(await c.call('Storage.clearDataForOrigin',{origin:O,storageTypes:'local_storage,indexeddb'})).error||'ok');await c.call('Page.bringToFront');await c.call('Page.navigate',{url:O+'/'});
await wait(1500);await c.call('Page.bringToFront');log('hidden',await ev(c,'document.hidden',5000));
await ev(c,`navigator.wakeLock.request('screen').then(l=>{window.__wl0=l;return 1}).catch(e=>String(e))`,5000).then(v=>log('wake lock',v));
const t0=Date.now();const T=()=>Math.round((Date.now()-t0)/1000);log('therm0',therm());
let first=null;
for(let i=0;i<360;i++){await wait(1000);let s;try{s=await ev(c,`({p:typeof pxl,c:!!document.querySelector('canvas'),l:document.body.innerText.includes('공간을 불러오고')})`,5000)}catch(e){continue}
 if(s.p==='object'&&s.c&&!s.l){first=T();break}}
log('world shown s',first);
if(first===null){log('state',await ev(c,`(async()=>JSON.stringify({text:document.body.innerText.slice(0,160),vis:document.visibilityState,perm:(await navigator.permissions.query({name:'geolocation'})).state,geo:await new Promise(res=>navigator.geolocation.getCurrentPosition(p=>res(['ok',p.coords.latitude]),e=>res(['err',e.code,e.message]),{timeout:5000})),pxl:typeof pxl,res:performance.getEntriesByType('resource').length,bad:performance.getEntriesByType('resource').filter(e=>e.responseStatus>=400||e.responseStatus===0).slice(-8).map(e=>[new URL(e.name).pathname.slice(-50),e.responseStatus])}))()`,15000).catch(e=>String(e)))}
if(first===null){try{await c.call('Page.navigate',{url:'about:blank'})}catch{};c.close();chrome?.kill();process.exit(1)}
await wait(15000);
const P='/@fs'+W;
log('page',await ev(c,`(async()=>{window.__wl=await navigator.wakeLock.request('screen').catch(()=>0);const S=await import('${P}/port/setting.ts');const Q=await import('${P}/shell/setting/schema.ts');const F=await import('${P}/port/perf.ts');window.__m={set:S.set,SET:Q.SETTING,start:F.start};const cv=document.querySelector('canvas');const a=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});return {vw:innerWidth,vh:innerHeight,dpr:devicePixelRatio,cv:[cv.width,cv.height],gpu:[a.info.vendor,a.info.architecture],ts:a.features.has('timestamp-query')}})()`));
await ev(c,`__m.set(__m.SET.perf.key,1)`);await wait(1000);await ev(c,`__m.start()`);await wait(5000);
let live;const t1=Date.now();for(let i=0;i<Number(process.env.WAIT||180);i++){live=await ev(c,`(()=>{const r=pxl.perf.report();const v=(r.match(/view +pivot ([^ ]+)/)||[])[1];const f=pxl.perf.read().frames||{};return {read:f.gpu_read,n:f.n,pivot:v||'-'}})()`);if(live.read>0&&live.pivot!=='-')break;if(i%30===0)await ev(c,`__m.start()`);await wait(1000)}
log('renderer live',{...live,waited_s:Math.round((Date.now()-t1)/1000)});
if(!(live.read>0&&live.pivot!=='-')){log('geo',await ev(c,`(async()=>{const st=(await navigator.permissions.query({name:'geolocation'})).state;const r=await new Promise(res=>navigator.geolocation.getCurrentPosition(p=>res(['ok',p.coords.latitude]),e=>res(['err',e.code,e.message]),{timeout:6000}));return {st,r}})()`,12000));log('invalid',await ev(c,`JSON.stringify({text:document.body.innerText.slice(0,200),bad:performance.getEntriesByType('resource').filter(e=>e.responseStatus>=400||e.responseStatus===0).slice(-12).map(e=>[new URL(e.name).pathname.slice(-70),e.responseStatus]),n:performance.getEntriesByType('resource').length})`));try{await c.call('Page.navigate',{url:'about:blank'})}catch{};c.close();chrome?.kill();process.exit(2)}
const motion=`(async(secs,turn)=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();const [x,y]=[r.left+r.width/2,r.top+r.height/2];const p=(t,a,b)=>cv.dispatchEvent(new PointerEvent(t,{pointerId:1,pointerType:'mouse',isPrimary:true,clientX:a[0],clientY:a[1],buttons:b,button:2,bubbles:true,cancelable:true}));const on=a=>[x+100*Math.cos(a),y+100*Math.sin(a)];p('pointerdown',on(-Math.PI/2),2);const t0=performance.now();const s=setInterval(()=>p('pointermove',on(-Math.PI/2+(performance.now()-t0)/1000/turn*2*Math.PI),2),16);await new Promise(r=>setTimeout(r,secs*1000));clearInterval(s);p('pointerup',on(-Math.PI/2),0);return 1})`;
const set=(k,val)=>ev(c,`__m.set(__m.SET.${k}.key,${val})`);
await set('perf',1);await set('ray_counts',0);await set('shadows',0);await set('res','__m.SET.res.options.fixed');await wait(4000);
const window_=async(name,secs)=>{await ev(c,`__m.start()`);await ev(c,motion+`(${secs},6)`,240000);log(name,await ev(c,`pxl.perf.report()`));log('therm '+name,therm())};
const view=()=>ev(c,`(pxl.perf.report().match(/distance [0-9.]+ u +polar [-0-9.]+. +azimuth [-0-9.]+/)||[''])[0]`);
const shot=async n=>{const r=await c.call('Page.captureScreenshot',{format:'png'});if(!r.error)fs.writeFileSync(`${n}.png`,Buffer.from(r.result.data,'base64'))};
const L=process.env.LABEL;
await ev(c,`(async()=>{const F=await import('${P}/port/perf.ts');__m.frames=F.perf_frames;return 1})()`);
await set('ray_counts',0);await set('shadows',1);await set('sky_time','__m.SET.sky_time.options.noon');await wait(10000);await shot(`pair-${L}-rest`);log('view rest',await view());
await ev(c,`__m.start()`);await ev(c,motion+`(25,6)`,240000);await shot(`pair-${L}-stop`);log('circle',await ev(c,`pxl.perf.report()`));log('view stop',await view());
const rows=await ev(c,`(()=>{const names=(pxl.perf.read().frames.gpu_spans||[]).map(x=>x.name);return JSON.stringify({names,rows:__m.frames().filter(f=>f.gpu_read).map(f=>[f.gpu_busy_ms,...f.gpu_spans])})})()`,60000);fs.writeFileSync(`pair-${L}-rows.json`,rows);
await wait(2000);await shot(`pair-${L}-stop2`);
await set('ray_counts',1);await wait(1000);await ev(c,`__m.start()`);await ev(c,motion+`(8,6)`,240000);log('counts',await ev(c,`pxl.perf.report()`));await set('ray_counts',0);log('therm',therm());log('errs',await ev(c,`JSON.stringify(window.__E)`,5000));
log('done',T());b.close();
if(D==='s23'){await c.call('Page.navigate',{url:'about:blank'});c.close();for(const x of (await (await fetch(`http://localhost:${PORT}/json`)).json()).filter(x=>x.type==='page'&&x.url==='about:blank'&&x.id!==t.id)){await fetch(`http://localhost:${PORT}/json/close/${x.id}`);log('closed a blank tab',1)}execSync(`${A} reverse --remove tcp:5174`);execSync(`${A} forward --remove tcp:9333`)}
else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}

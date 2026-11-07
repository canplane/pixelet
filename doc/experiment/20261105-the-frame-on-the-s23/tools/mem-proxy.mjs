// node mem-proxy.mjs UPSTREAM LISTEN : the page on UPSTREAM served again on LISTEN, its HTML given a first script that
// records errors into window.__E, keeps the device as window.__dev, counts every GPU buffer and texture made and
// destroyed (live bytes), the queue's submits against the work done, and beacons them to :8099 each second, with the
// largest allocations at 5, 20, 45 and 90 s and at the device's loss.
import http from 'node:http';
const UP=Number(process.argv[2]||8083),PORT=Number(process.argv[3]||8093);
const HOOK=`<script>(()=>{window.__E=[];const push=(k,a)=>{if(window.__E.length<200)window.__E.push(k+': '+a.map(x=>String(x&&x.message||x)).join(' ').slice(0,600))};
for(const k of ['error','warn']){const f=console[k];console[k]=(...a)=>{push(k,a);f.apply(console,a)}}
addEventListener('error',e=>push('page',[e.message]));addEventListener('unhandledrejection',e=>push('rejection',[e.reason]));
const pos=()=>({coords:{latitude:38.1193,longitude:128.4656,accuracy:10,altitude:null,altitudeAccuracy:null,heading:null,speed:null},timestamp:Date.now()});
const g=navigator.geolocation;let id=1;g.getCurrentPosition=(ok)=>{setTimeout(()=>ok(pos()),0)};g.watchPosition=(ok)=>{setTimeout(()=>ok(pos()),0);return id++};g.clearWatch=()=>{};
const BPP={r8unorm:1,r8uint:1,rg8unorm:2,r16float:2,r16uint:2,r32float:4,r32uint:4,r32sint:4,rg16float:4,rgba8unorm:4,'rgba8unorm-srgb':4,bgra8unorm:4,'bgra8unorm-srgb':4,rgb10a2unorm:4,rg11b10ufloat:4,rg32float:8,rg32uint:8,rgba16float:8,rgba16uint:8,rgba32float:16,rgba32uint:16,depth32float:4,depth24plus:4,'depth24plus-stencil8':4,'depth32float-stencil8':5,depth16unorm:2};
const M={live:0,peak:0,n:0,made:0,gone:0,submits:0,done:0,list:new Map()};window.__M=M;
const B=(k,v)=>{try{navigator.sendBeacon('http://localhost:8099/'+k,JSON.stringify(v))}catch{}};
const track=(o,bytes,label,kind)=>{M.live+=bytes;M.made+=bytes;M.n++;if(M.live>M.peak)M.peak=M.live;M.list.set(o,[kind,label||'?',bytes]);const ds=o.destroy.bind(o);o.destroy=()=>{if(M.list.has(o)){M.live-=bytes;M.gone+=bytes;M.n--;M.list.delete(o)}return ds()};return o};
const top=()=>[...M.list.values()].sort((x,y)=>y[2]-x[2]).slice(0,25).map(x=>x.join(' '));window.__top=top;
const wrap=d=>{const cb=d.createBuffer.bind(d);d.createBuffer=desc=>track(cb(desc),desc.size,desc.label,'buf');
const ct=d.createTexture.bind(d);d.createTexture=desc=>{const s=desc.size;const w=s.width??s[0],h=(s.height??s[1])||1,l=(s.depthOrArrayLayers??s[2])||1;const lv=desc.mipLevelCount||1;let px=0;for(let i=0;i<lv;i++)px+=Math.max(1,w>>i)*Math.max(1,h>>i)*(desc.dimension==='3d'?Math.max(1,l>>i):l);return track(ct(desc),px*(BPP[desc.format]||4)*(desc.sampleCount||1),(desc.label||'?')+' '+desc.format+' '+w+'x'+h+'x'+l+(lv>1?' m'+lv:''),'tex')};
const q=d.queue;const sub=q.submit.bind(q);q.submit=a=>{M.submits++;const r=sub(a);q.onSubmittedWorkDone().then(()=>M.done++);return r};
d.lost.then(i=>B('lost',{reason:i.reason,message:i.message,live:M.live,peak:M.peak,n:M.n,submits:M.submits,done:M.done,top:top()}));
setInterval(()=>B('m',{live:M.live,peak:M.peak,n:M.n,made:M.made,gone:M.gone,inflight:M.submits-M.done}),1000)};
if(window.GPUAdapter){const rd=GPUAdapter.prototype.requestDevice;GPUAdapter.prototype.requestDevice=function(...a){return rd.apply(this,a).then(d=>{window.__dev=d;wrap(d);d.addEventListener('uncapturederror',e=>push('gpu',[e.error]));d.lost.then(i=>push('lost',[i.reason,i.message]));return d})}}else push('nogpu',['no GPUAdapter'])})();</script>`;
http.createServer((req,res)=>{
  const up=http.request({host:'localhost',port:UP,path:req.url,method:req.method,headers:{...req.headers,host:`localhost:${UP}`,...(/^\/(tile|asset)\//.test(req.url)?{}:{'accept-encoding':'identity'})}},r=>{
    const html=(r.headers['content-type']||'').includes('text/html');
    if(!html||r.headers['content-encoding']){res.writeHead(r.statusCode,r.headers);r.pipe(res);return}
    const parts=[];r.on('data',d=>parts.push(d));r.on('end',()=>{const body=Buffer.concat(parts).toString().replace(/<head[^>]*>/i,m=>m+HOOK);const h={...r.headers};delete h['content-length'];res.writeHead(r.statusCode,h);res.end(body)});
  });
  up.on('error',e=>{res.writeHead(502);res.end(String(e))});req.pipe(up);
}).listen(PORT,()=>console.log(`mem proxy ${PORT} -> ${UP}`));

// node visit.mjs s23|mac [--dump dir] : what one visit of the walk costs, a factor at a time.
// The real pass's size: workgroup 8x8, dispatch 28x49, 14 steps, 1.23M visits, over a
// table of 2^19 entries of 12 words (25 MB). A hit at the first slot is 13 loads
// (the key, 4 node words, 4 of what was handed, the kind, 2 of the asset, the key again).
import {client,ev} from './cdp.mjs';
import {execSync,spawn} from 'node:child_process';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';import http from 'node:http';
const ENTRIES=1<<19,ENTRY=12,STEPS=14,COLD=2000,GROUPS=[28,49];
function wgsl({mem,addr,stack,cold}){
  const load={none:()=>`0u`,atomic_rw:w=>`atomicLoad(&t.v[${w}])`,plain_rw:w=>`t.v[${w}]`,plain_ro:w=>`t.v[${w}]`,keys_atomic:w=>`t.v[${w}]`}[mem];
  const key=mem==='keys_atomic'?e=>`atomicLoad(&k.v[${e}])`:e=>load(`${e} * ${ENTRY}u`);
  const decl={none:``,
    atomic_rw:`struct T { v: array<atomic<u32>> }\n@group(0) @binding(2) var<storage, read_write> t: T;`,
    plain_rw:`struct T { v: array<u32> }\n@group(0) @binding(2) var<storage, read_write> t: T;`,
    plain_ro:`struct T { v: array<u32> }\n@group(0) @binding(2) var<storage, read> t: T;`,
    keys_atomic:`struct T { v: array<u32> }\n@group(0) @binding(2) var<storage, read> t: T;\nstruct K { v: array<atomic<u32>> }\n@group(0) @binding(3) var<storage, read_write> k: K;`}[mem];
  const coldVars=cold?Array.from({length:COLD},(_,i)=>`  var p${i}: u32;`).join('\n'):'';
  const coldLoop=cold?`    if u.cold != 0u {\n${Array.from({length:COLD},(_,i)=>`      p${i} = (p${i?i-1:COLD-1} * 3u + a)${i%2?' ^ h':''};`).join('\n')}\n    }`:'';
  const coldEnd=cold?`  if u.cold != 0u {\n${Array.from({length:COLD},(_,i)=>`    a ^= p${i};`).join('\n')}\n  }`:'';
  const visit=mem==='none'?`    let inh = h >> 3u;\n    hits += 1u;\n    a ^= inh;`:`    let e = h & ${ENTRIES-1}u & ~3u;
    let w = e * ${ENTRY}u;
    let key = ${key('e')};
    var inh = 0u;
    if key != 0u && ${load('w + 8u')} == word(w + 8u) && ${load('w + 9u')} == word(w + 9u) && ${load('w + 10u')} == word(w + 10u) && ${load('w + 11u')} == word(w + 11u) {
      inh = ${load('w + 4u')};
      a ^= ${load('w + 5u')} + ${load('w + 6u')} + ${load('w + 7u')};
      a += ${load('w + 1u')} ^ ${load('w + 2u')} ^ ${load('w + 3u')};
      if ${key('e')} == key { hits += 1u; }
    }`;
  const start=addr==='random'?`vec3u(id.x * 2654435761u, id.y * 2246822519u, 7u)`:`vec3u(id.x >> 5u, id.y >> 5u, 7u)`;
  return `struct U { cold: u32, pad: u32, pad1: u32, pad2: u32 }
@group(0) @binding(0) var<uniform> u: U;
struct Sink { v: array<u32> }
@group(0) @binding(1) var<storage, read_write> sink: Sink;
${decl}
struct Handed { a: u32, b: u32, c: u32, d: u32 }
fn mix(h: u32, v: u32) -> u32 { let m = (h ^ v) * 0x9E3779B1u; return (m << 15u) | (m >> 17u); }
fn word(w: u32) -> u32 { var x = (w + 1u) * 2654435761u; x ^= x >> 15u; x *= 2246822519u; x ^= x >> 13u; return x | 1u; }
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3u) {
  var c = ${start};
  var a = 0u;
  var hits = 0u;
${coldVars}
${stack?`  var stack: array<Handed, 33>;\n  let first = Handed(c.x, c.y, c.z, 1u);\n${Array.from({length:33},(_,i)=>`  stack[${i}u] = first;`).join('\n')}`:''}
  var step = 0u;
  loop {
    if step >= ${STEPS}u { break; }
    var h = step * 0x85EBCA77u;
    h = mix(h, c.x); h = mix(h, c.y); h = mix(h, c.z);
    h ^= h >> 16u;
${visit}
${stack?`    stack[(step + (inh & 3u)) & 31u] = Handed(inh, h, a, step);\n    let over = stack[(h >> 7u) & 31u];\n    a ^= over.b;`:''}
    c = vec3u(c.x + (inh & 1u), c.y + ((inh >> 1u) & 1u), c.z + 1u);
${coldLoop}
    step += 1u;
  }
${coldEnd}
  let i = (id.y * ${GROUPS[0]*8}u + id.x) * 2u;
  sink.v[i] = a;
  sink.v[i + 1u] = hits;
}
`}
const base={addr:'coarse',stack:false,cold:false};
const V=[['none',{mem:'none'}],['atomic_rw',{mem:'atomic_rw'}],['plain_rw',{mem:'plain_rw'}],['plain_ro',{mem:'plain_ro'}],['keys_atomic',{mem:'keys_atomic'}],
 ['atomic_rw random',{mem:'atomic_rw',addr:'random'}],['plain_ro random',{mem:'plain_ro',addr:'random'}],
 ['atomic_rw stack',{mem:'atomic_rw',stack:true}],['plain_ro stack',{mem:'plain_ro',stack:true}],
 ['none cold',{mem:'none',cold:true}],['atomic_rw cold',{mem:'atomic_rw',cold:true}],['plain_ro cold',{mem:'plain_ro',cold:true}],
 ['atomic_rw stack cold',{mem:'atomic_rw',stack:true,cold:true}],['plain_ro stack cold',{mem:'plain_ro',stack:true,cold:true}]].map(([n,o])=>({name:n,mem:o.mem,code:wgsl({...base,...o})}));
if(process.argv[3]==='--dump'){const d=process.argv[4];fs.mkdirSync(d,{recursive:true});for(const v of V)fs.writeFileSync(path.join(d,v.name.replaceAll(' ','-')+'.wgsl'),v.code);console.log(V.length,'written');process.exit(0)}
const page=`async (V,ROUNDS,REPS,DISPATCHES)=>{
 const a=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});const d=await a.requestDevice({requiredFeatures:['timestamp-query'],requiredLimits:{maxStorageBufferBindingSize:a.limits.maxStorageBufferBindingSize}});
 let lost=null;d.lost.then(i=>lost=i.message);
 const E=${ENTRIES},W=${ENTRY};
 const word=w=>{let x=Math.imul(w+1,2654435761)>>>0;x^=x>>>15;x=Math.imul(x,2246822519)>>>0;x^=x>>>13;return (x|1)>>>0};
 const tb=d.createBuffer({size:E*W*4,usage:GPUBufferUsage.STORAGE,mappedAtCreation:true});const m=new Uint32Array(tb.getMappedRange());
 for(let i=0;i<m.length;i++)m[i]=word(i);tb.unmap();
 const kb=d.createBuffer({size:E*4,usage:GPUBufferUsage.STORAGE,mappedAtCreation:true});const km=new Uint32Array(kb.getMappedRange());for(let e=0;e<E;e++)km[e]=word(e*W);kb.unmap();
 const ub=d.createBuffer({size:16,usage:GPUBufferUsage.UNIFORM,mappedAtCreation:true});ub.unmap();
 const N=${GROUPS[0]*8}*${GROUPS[1]*8};
 const sb=d.createBuffer({size:N*8,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_SRC});
 const rb=d.createBuffer({size:N*8,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST});
 const qs=d.createQuerySet({type:'timestamp',count:2});const qb=d.createBuffer({size:16,usage:GPUBufferUsage.QUERY_RESOLVE|GPUBufferUsage.COPY_SRC});const qr=d.createBuffer({size:16,usage:GPUBufferUsage.MAP_READ|GPUBufferUsage.COPY_DST});
 const P=[];
 for(const v of V){const t0=performance.now();const mod=d.createShaderModule({code:v.code});let pipe=null,err=null;d.pushErrorScope('validation');try{pipe=await d.createComputePipelineAsync({layout:'auto',compute:{module:mod,entryPoint:'main'}})}catch(e){err=String(e.message).slice(0,160)}const ve=await d.popErrorScope();if(ve)err=(err||'')+ve.message.slice(0,160);
  const ms=performance.now()-t0;let group=null;if(pipe){const es=[{binding:1,resource:{buffer:sb}}];if(v.code.includes('@binding(0) var<uniform> u')&&v.code.includes('u.cold'))es.unshift({binding:0,resource:{buffer:ub}});if(v.mem!=='none')es.push({binding:2,resource:{buffer:tb}});if(v.mem==='keys_atomic')es.push({binding:3,resource:{buffer:kb}});group=d.createBindGroup({layout:pipe.getBindGroupLayout(0),entries:es})}
  P.push({name:v.name,pipe,group,ms:Math.round(ms),err,t:[],wall:[],hits:null,d:1,lost:false})}
 const run=async(p,read)=>{const w0=performance.now();const en=d.createCommandEncoder();const pass=en.beginComputePass({timestampWrites:{querySet:qs,beginningOfPassWriteIndex:0,endOfPassWriteIndex:1}});pass.setPipeline(p.pipe);pass.setBindGroup(0,p.group);for(let n=0;n<p.d;n++)pass.dispatchWorkgroups(${GROUPS[0]},${GROUPS[1]},1);pass.end();en.resolveQuerySet(qs,0,2,qb,0);en.copyBufferToBuffer(qb,0,qr,0,16);if(read)en.copyBufferToBuffer(sb,0,rb,0,N*8);d.queue.submit([en.finish()]);await d.queue.onSubmittedWorkDone();await qr.mapAsync(GPUMapMode.READ);const q=new BigUint64Array(qr.getMappedRange());const ns=Number(q[1]-q[0]);qr.unmap();p.wall.push((performance.now()-w0)/p.d);return ns/p.d};
 for(let r=0;r<ROUNDS;r++)for(const p of P){if(!p.pipe)continue;try{const t1=await run(p,false);p.d=Math.max(1,Math.min(DISPATCHES,Math.round(50e6/Math.max(t1,1e4))));const w=performance.now();while(performance.now()-w<1000)await run(p,false);p.wall.length=0;for(let k=0;k<REPS;k++){p.t.push(await run(p,false));p.allwall=(p.allwall||[]).concat(p.wall.splice(0))}}catch(e){p.lost=String(e.message).slice(0,120);return JSON.stringify({lost:lost||p.lost,adapter:[a.info.vendor,a.info.architecture],stopped:p.name,res:P.map(p=>({name:p.name,compile_ms:p.ms,err:p.err,hits:p.hits,d:p.d,lostHere:p.lost,t:p.t,wall:p.allwall||[]}))})}
  if(r===0){await run(p,true);await rb.mapAsync(GPUMapMode.READ);const h=new Uint32Array(rb.getMappedRange());let lo=1e9,hi=0;for(let i=1;i<h.length;i+=2){if(h[i]<lo)lo=h[i];if(h[i]>hi)hi=h[i]}rb.unmap();p.hits=[lo,hi]}}
 return JSON.stringify({lost,adapter:[a.info.vendor,a.info.architecture],res:P.map(p=>({name:p.name,compile_ms:p.ms,err:p.err,hits:p.hits,d:p.d,t:p.t,wall:p.allwall||[]}))})
}`;
const DISP=Number(process.env.DISP||20);const D=process.argv[2];const PORT=D==='s23'?9333:9222;const A=process.env.HOME+'/Library/Android/sdk/platform-tools/adb';const wait=ms=>new Promise(r=>setTimeout(r,ms));const PAGE=5175;
const server=http.createServer((_,res)=>{res.writeHead(200,{'content-type':'text/html'});res.end('<!doctype html><title>visit</title><body>visit</body>')}).listen(PAGE);
let chrome,dir;
if(D==='s23'){execSync(`${A} reverse tcp:${PAGE} tcp:${PAGE}`);execSync(`${A} forward tcp:9333 localabstract:chrome_devtools_remote`);execSync(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d http://localhost:${PAGE}/`)}
else{dir=fs.mkdtempSync(path.join(os.tmpdir(),'pxl-visit-'));chrome=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[`--user-data-dir=${dir}`,'--remote-debugging-port=9222','--no-first-run','--no-default-browser-check','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding','--disable-background-timer-throttling',`http://localhost:${PAGE}/`],{stdio:'ignore'})}
let t;for(let i=0;i<30&&!t;i++){await wait(1000);try{t=(await (await fetch(`http://localhost:${PORT}/json`)).json()).find(x=>x.type==='page'&&x.url.startsWith(`http://localhost:${PAGE}`))}catch{}}if(!t)throw new Error('no page tab');
const c=client(t.webSocketDebuggerUrl);await c.open;await c.call('Page.enable');await c.call('Page.bringToFront');
console.log('wake',await ev(c,`navigator.wakeLock.request('screen').then(l=>{window.__w=l;return 'held'}).catch(e=>String(e))`,8000));
let left=V;const r={res:[],lostAt:[]};
while(left.length){const o=JSON.parse(await ev(c,`(${page})(${JSON.stringify(left)},${Number(process.env.ROUNDS||3)},${Number(process.env.REPS||12)},${DISP})`,900000));r.adapter=o.adapter;r.lost=o.lost;
 if(!o.stopped){r.res.push(...o.res);break}
 console.log('lost at',o.stopped,o.lost);r.lostAt.push(o.stopped);const at=left.findIndex(v=>v.name===o.stopped);r.res.push(...o.res.slice(0,at).map(p=>p),{...o.res[at],err:'device lost'});left=left.slice(at+1);
 await c.call('Page.reload',{});await wait(3000);await ev(c,`navigator.wakeLock.request('screen').then(l=>{window.__w=l;return 1}).catch(()=>0)`,8000)}fs.writeFileSync(`visit-${D}.json`,JSON.stringify(r,null,1));
const med=a=>{if(!a.length)return NaN;const s=[...a].sort((x,y)=>x-y);return s[(s.length-1)>>1]};const visits=GROUPS[0]*8*GROUPS[1]*8*STEPS;
console.log(r.adapter,'lostAt',r.lostAt,'visits a dispatch',visits);
for(const p of r.res)console.log(p.name.padEnd(24),p.err?'ERR '+p.err:`gpu ${(med(p.t)/visits).toFixed(2).padStart(7)} ns/visit  wall ${(med(p.wall)*1e6/visits).toFixed(2).padStart(7)}  ms ${(med(p.t)/1e6).toFixed(1).padStart(8)}  rounds ${(med(p.t.slice(0,p.t.length/3))/visits).toFixed(2)} ${(med(p.t.slice(p.t.length/3,2*p.t.length/3))/visits).toFixed(2)} ${(med(p.t.slice(-p.t.length/3))/visits).toFixed(2)}  hits ${p.hits}  d ${p.d}  compile ${p.compile_ms} ms`);
if(D==='s23'){try{await c.call('Page.navigate',{url:'about:blank'})}catch{};c.close();execSync(`${A} reverse --remove tcp:${PAGE}`);execSync(`${A} forward --remove tcp:9333`)}else{c.close();chrome.kill();await wait(1500);fs.rmSync(dir,{recursive:true,force:true})}
server.close();process.exit(0);

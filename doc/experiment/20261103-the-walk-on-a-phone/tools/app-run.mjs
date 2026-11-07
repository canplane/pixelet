import {spawn,execSync} from 'node:child_process';import fs from 'node:fs';
const A=process.env.HOME+'/Library/Android/sdk/platform-tools/adb';const out=fs.createWriteStream('app-run.log');
execSync(`${A} logcat -c`);execSync(`${A} shell am start -S -n me.plei.pixelet/.MainActivity`);
const l=spawn(A,['logcat','-v','time','ReactNativeJS:V','pxl:V','AndroidRuntime:E','wgpu:V','*:S']);let buf='';
const end=c=>{l.kill();out.end(()=>process.exit(c))};setTimeout(()=>{console.log('timeout');end(2)},Number(process.env.SECS||420)*1000);
l.stdout.on('data',d=>{out.write(d);buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const s=buf.slice(0,i);buf=buf.slice(i+1);
 if(/PROBE (boot|world|failed)|FATAL|panicked/.test(s))console.log(s.slice(0,170));
 if(/PROBE shot\|/.test(s))try{fs.writeFileSync('app-shot.png',execSync(`${A} exec-out screencap -p`,{maxBuffer:1<<26}))}catch{}
 if(/PROBE done\|/.test(s)){console.log('done');end(0)}}});

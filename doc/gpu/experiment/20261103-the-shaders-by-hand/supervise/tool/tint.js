import { createRequire } from 'node:module';
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
const root = execFileSync('git', ['-C', import.meta.dirname, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
const require = createRequire(root + '/ts/app/web/src/x.js');
const { create, globals } = require('webgpu');
Object.assign(globalThis, globals);
const gpu = create([]);
const adapter = await gpu.requestAdapter();
console.log('adapter', adapter?.info?.vendor, adapter?.info?.architecture, adapter?.info?.description);
const device = await adapter.requestDevice();
for (const f of process.argv.slice(2)) {
  const code = fs.readFileSync(f, 'utf8');
  const entry = code.match(/@compute[^]*?\bfn\s+(\w+)\s*\(/)?.[1];
  device.pushErrorScope('validation');
  const t0 = Date.now();
  const module = device.createShaderModule({ code });
  const info = await module.getCompilationInfo();
  console.log(f.split('/').pop(), 'messages', info.messages.length);
  for (const m of info.messages.slice(0, 20)) console.log(' ', m.type, m.lineNum, m.linePos, m.message.split('\n')[0]);
  let p;
  try { p = await device.createComputePipelineAsync({ layout: 'auto', compute: { module, entryPoint: entry } }); } catch (e) { console.log('pipeline refused', e.message); }
  const scoped = await device.popErrorScope();
  console.log(entry, 'pipeline', p ? 'made' : 'none', 'scope', scoped ? scoped.message.split('\n')[0] : 'clean', (Date.now() - t0) + 'ms');
}
device.destroy();
void gpu;
process.exit(0);

# python3 -I trace.py TRACE.json : a profiler trace's long frames (over 2.5 refreshes) and what the
# three worst hold: their own turn, the GPU reads about them with their largest spans, the browser's
# long animation frames over them (script, render, blocking) and the kernel's tasks then.
import json, sys

K = {0: 'none', 1: 'unready', 2: 'held', 3: 'dropped', 4: 'plain', 5: 'standing', 6: 'cast', 7: 'stood', 8: 'px', 9: 'gated'}
d = json.load(open(sys.argv[1])); ev = d['traceEvents']
fr = sorted((e for e in ev if e['ph'] == 'C' and e['name'] == 'frame'), key=lambda e: e['ts'])
loaf = [e for e in ev if e.get('cat') in ('loaf', 'longtask')]
tasks = [e for e in ev if e.get('cat', '').startswith('task') and e['name'] != 'run']
Q = lambda a, q: a[min(len(a) - 1, int(len(a) * q))]
iv = sorted(f['args']['interval_ms'] for f in fr)
ref = Q(sorted(f['args']['refresh_ms'] for f in fr), .5)
long = [f for f in fr if f['args']['interval_ms'] > 2.5 * ref]
print(f'{len(fr)} frames, interval p50 {Q(iv,.5)} p95 {Q(iv,.95)} max {iv[-1]}; {len(long)} over 2.5 refreshes hold '
      f'{sum(f["args"]["interval_ms"] for f in long)/1000:.1f} s of {(fr[-1]["ts"]-fr[0]["ts"])/1e6:.1f} s')
gb = sorted(f['args']['gpu_busy_ms'] for f in fr if f['args']['gpu_read'])
print(f'GPU busy p50 {Q(gb,.5)} p95 {Q(gb,.95)}, summed {sum(gb)/1000:.1f} s over {len(gb)} reads')
big = [l for l in loaf if l['dur'] > 100000 and 'scriptMs' in l.get('args', {})]
if big:
    m = lambda k: sum(l['args'][k] for l in big) / len(big)
    print(f'long animation frames over 100 ms: {len(big)}, mean {sum(l["dur"] for l in big)/len(big)/1000:.0f} ms, '
          f'script {m("scriptMs"):.1f}, render {m("renderMs"):.1f}, blocking {m("blockingMs"):.1f}')
spans = [k for k in fr[0]['args'] if k.startswith('gpu_spans_')]
for f in sorted(long, key=lambda f: -f['args']['interval_ms'])[:3]:
    a = f['args']; t1 = f['ts']; t0 = t1 - a['interval_ms'] * 1000; i = fr.index(f)
    print(f'\nframe at {a["t"]:.0f} ms: interval {a["interval_ms"]} ms, {K.get(a["kind_name"])}, turns {a["turns"]}, '
          f'CPU busy {a["busy_ms"]} (tick {a["tick_ms"]}, stage {a["stage_ms"]})')
    for g in fr[max(0, i - 4): i + 4]:
        b = g['args']; top = sorted(((b[k], k[10:]) for k in spans if b[k] > 0), reverse=True)[:4]
        print(f'  t {b["t"]:.0f} iv {b["interval_ms"]:>5} {K.get(b["kind_name"]):6} read {b["gpu_read"]} gpu {b["gpu_busy_ms"]:>6.1f} '
              + ' '.join(f'{n} {v:.1f}' for v, n in top) + (' <' if g is f else ''))
    for l in loaf:
        if l['ts'] < t1 and l['ts'] + l['dur'] > t0:
            print(f'  {l["name"]}: {json.dumps(l.get("args", {}))}')
    for tk in tasks:
        if tk['ts'] < t1 and tk['ts'] + tk['dur'] > t0:
            print(f'  task {tk["name"]} {tk["cat"]} {tk["dur"]/1000:.0f} ms')

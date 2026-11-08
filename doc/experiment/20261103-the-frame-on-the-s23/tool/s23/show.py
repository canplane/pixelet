# python3 -I show.py SPANS NAME:LABEL,LABEL NAME:LABEL,LABEL … : runs side by side, from OUT (default
# <repo>/cache/s23): each circle's and walk's fps, frame GPU busy p50/p95 and each named span's p50/p95
# (SPANS comma-separated); the drive's fps, its cast frames' busy and the spans' p95 as the report
# gives them; the eye's and the light's nodes a cast from the counts windows; errors.
import json, os, re, sys

OUT = os.environ.get('OUT') or os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../../../cache/s23')
SP = sys.argv[1].split(',')
groups = [a.split(':') for a in sys.argv[2:]]
Q = lambda a, q: a[min(len(a) - 1, int(len(a) * q))] if a else float('nan')

def load(L):
    return dict((k, v) for k, v in json.load(open(os.path.join(OUT, L + '.json')))['log'])

for scene, key in (('off', 'circle off'), ('on', 'circle on'), ('walk', 'walk')):
    lines = []
    for name, labels in groups:
        for L in labels.split(','):
            d = load(L)
            if key not in d:
                continue
            R = json.load(open(os.path.join(OUT, f'{L}-{scene}-rows.json'))); n = R['names']; rows = R['rows']
            f = re.search(r'fps ([0-9.]+)', d[key]).group(1); b = sorted(r[0] for r in rows)
            spans = []
            for s in SP:
                if s in n:
                    i = 1 + n.index(s); x = sorted(r[i] for r in rows if r[i] > 0)
                    if x:
                        spans.append(f'{s} {Q(x,.5):.1f}/{Q(x,.95):.1f}')
            lines.append(f'  {name:8} {L:14} fps {f} busy {Q(b,.5):.1f}/{Q(b,.95):.1f} | ' + ' '.join(spans))
    if lines:
        print('==', scene); print('\n'.join(lines))

lines = []
for name, labels in groups:
    for L in labels.split(','):
        r = load(L).get('drive')
        if not r:
            continue
        f = re.search(r'fps ([0-9.]+)', r); c = re.search(r'busy by kind\s+cast p50 ([0-9.]+) p95 ([0-9.]+)', r)
        sp = re.search(r'spans +p95 (.*?) ms', r)
        keep = ' '.join(x for x in (sp.group(1).split('  ') if sp else []) if any(x.startswith(k + ' ') for k in SP))
        lines.append(f'  {name:8} {L:14} fps {f and f.group(1)} cast p50/p95 {c and c.groups()} | {keep}')
if lines:
    print('== drive'); print('\n'.join(lines))

for name, labels in groups:
    for L in labels.split(','):
        d = load(L)
        for scene in ('off', 'on'):
            for l in d.get('counts ' + scene, '').split('\n'):
                m = re.search(r'(eye|light) +cast in (\d+) frames +nodes a cast: beam ([0-9.]+) +sample ([0-9.]+) +miss ([0-9.]+)', l)
                if m:
                    print(f'  nodes {name:8} {L:14} {scene:3} {m.group(1):5} beam {float(m.group(3)):.0f} sample {float(m.group(4)):.0f} miss {float(m.group(5)):.0f}')
        e = d.get('errs')
        if e and e != '[]':
            print(f'  errs {L}: {e[:200]}')

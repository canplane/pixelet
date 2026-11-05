import re
from collections import defaultdict
S=defaultdict(list)
for l in open('app-run.log',errors='replace'):
    m=re.search(r'PROBE (\w+)\|(.*)',l)
    if m:S[m.group(1)].append(m.group(2).strip('"\' \n'))
for k in('boot','world','off','on','counts','drive'):
    if k in S:
        print('==',k)
        for x in S[k]:
            t=x.strip()
            if k in('boot','world') or re.match(r'frames n=|kinds|gpu  |eye  |light  |view  ',t):print(' ',t[:230])
            if t.startswith('spans'):
                sp=sorted([(float(b),a) for a,b in re.findall(r'(\w+) ([0-9.]+) \(\d+\)',t)],reverse=True)[:7];print('  top',[(a,b) for b,a in sp])

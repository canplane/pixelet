import json,statistics as st,sys
for n in sys.argv[1:]:
    try:J=json.load(open(f'frames-s23-{n}.json'))
    except Exception as e:print(n,'missing');continue
    names=J['names'];R=J['rows'];G=[r for r in R if r[2]]
    s=sorted(r[3] for r in G);iv=sorted(r[1] for r in R);N=lambda a,q:a[min(len(a)-1,int(len(a)*q))]
    span=lambda m:round(st.mean(r[10+names.index(m)] for r in G),1)
    secs=(R[-1][0]-R[0][0])/1000
    print(f"{n:11} frames {len(R):4} fps {len(R)/secs:5.1f} busy p50 {N(s,.5):5.1f} p90 {N(s,.9):6.1f} p95 {N(s,.95):6.1f} max {s[-1]:6.1f} | interval p50 {N(iv,.5):4.0f} p90 {N(iv,.9):4.0f} p95 {N(iv,.95):4.0f} max {iv[-1]:4.0f} | mean beam {span('beam')} sample {span('sample')} miss {span('miss')} world {span('world')} face {span('face')} generate {span('generate')}")

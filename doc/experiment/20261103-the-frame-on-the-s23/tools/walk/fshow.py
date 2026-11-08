import json,statistics as st,sys
for n in sys.argv[1:]:
    try:J=json.load(open(f'frames-s23-{n}.json'))
    except Exception as e:print(n,'missing');continue
    names=J['names'];R=J['rows'];A=[r for r in R if r[2] and r[5]];B=names.index('beam')
    z=[r[6] for r in A];bins=[0,20e3,100e3,250e3,500e3,1e9]
    h=[sum(1 for x in z if bins[i]<=x<bins[i+1]) for i in range(5)]
    s=sorted(r[3] for r in R if r[2])
    print(n,'frames both',len(A),'beam-lane bins <20k,<100k,<250k,<500k,>=500k',h,'mean lane %.0f'%st.mean(z),'p90 %.0f max %.0f'%(sorted(z)[int(len(z)*.9)],max(z)),'busy p50 %.1f p90 %.1f p95 %.1f'%(s[len(s)//2],s[int(len(s)*.9)],s[int(len(s)*.95)]),'world %.1f face %.1f'%(st.mean(r[10+names.index('world')] for r in A),st.mean(r[10+names.index('face')] for r in A)))

import json,sys,statistics as st
d=json.load(open(sys.argv[1]));R=d['rows']
print('rows',len(R))
# t,interval,read,busy,gpuframe,cast,beam,sample,miss,settled,span beam,sample_hit,sample,miss,face,generate,world
read=[r for r in R if r[2]]
print('with a gpu read',len(read),'with rays',sum(1 for r in R if r[5]))
ra=[r for r in R if r[5]]
print('rays rows sample',[ (round(r[6]),round(r[7]),round(r[8])) for r in ra[:8]])
pair=[r for r in R if r[2] and r[5]]
print('rows with both',len(pair))
def corr(a,b):
    if len(a)<3 or st.pstdev(a)==0 or st.pstdev(b)==0:return None
    ma,mb=st.mean(a),st.mean(b);return sum((x-ma)*(y-mb) for x,y in zip(a,b))/len(a)/st.pstdev(a)/st.pstdev(b)
idx=[i for i,r in enumerate(R)]
for off in range(-6,7):
    xs=[];ys=[];zs=[]
    for i,r in enumerate(R):
        j=i+off
        if r[5] and 0<=j<len(R) and R[j][2]:xs.append(r[6]);ys.append(R[j][3]);zs.append(R[j][10])
    print('offset',off,'n',len(xs),'corr left beam nodes vs busy',corr(xs,ys) and round(corr(xs,ys),2),'vs beam span',corr(xs,zs) and round(corr(xs,zs),2))
b=sorted(r[3] for r in read);print('busy p10,p50,p90,p95,max',[round(b[int(len(b)*q)],1) for q in(.1,.5,.9,.95)],round(b[-1],1))
s=sorted(r[1] for r in R);print('interval p50,p90,p95,max',[s[int(len(s)*q)] for q in(.5,.9,.95)],s[-1])
tops=sorted(read,key=lambda r:-r[3])[:10];print('worst frames busy,beam,sample_hit,sample,miss,face,generate,world');[print(' ',round(r[3],1),[round(x,1) for x in r[10:]]) for r in tops]
med=[r for r in read if abs(r[3]-b[len(b)//2])<3][:3];print('median frames');[print(' ',round(r[3],1),[round(x,1) for x in r[10:]]) for r in med]
for nm,k in (('beam',10),('sample',12),('miss',13),('face',14),('generate',15)):
    v=[r[3] for r in read];w=[r[k] for r in read];print(nm,'span share of the busiest 10%: mean',round(st.mean(sorted(zip(v,w))[-len(v)//10:][i][1] for i in range(len(v)//10)),1),'all mean',round(st.mean(w),1))

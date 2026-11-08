import sys,pickle
def varint(b,i):
    r=s=0
    while True:
        c=b[i];i+=1;r|=(c&127)<<s;s+=7
        if c<128:return r,i
def fields(b):
    i=0;n=len(b)
    while i<n:
        t,i=varint(b,i);f=t>>3;w=t&7
        if w==0:v,i=varint(b,i)
        elif w==2:
            l,i=varint(b,i);v=b[i:i+l];i+=l
        elif w==1:v=b[i:i+8];i+=8
        elif w==5:v=b[i:i+4];i+=4
        yield f,v
def packed(b):
    i=0;o=[]
    while i<len(b):
        v,i=varint(b,i);o.append(v)
    return o
def load(p):
    data=open(p,'rb').read();docs={}
    for f,v in fields(data):
        if f!=2:continue
        path=None;occ=[]
        for g,w in fields(v):
            if g==1:path=w.decode()
            elif g==2:
                r=None;s='';ro=0;e=None
                for h,x in fields(w):
                    if h==1:r=packed(x)
                    elif h==2:s=x.decode()
                    elif h==3:ro=x
                    elif h==7:e=packed(x)
                occ.append((r[0],r[1],s,ro,e))
        docs[path]=occ
    return docs

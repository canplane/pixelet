#!/bin/zsh
S=/private/tmp/claude-501/-Users-sanghoon-git-canplane-pixelet/572d46ae-efea-42fb-a49d-2e5909f1eb71/scratchpad
A=~/Library/Android/sdk/platform-tools/adb
cd "$S"
$A shell input keyevent KEYCODE_WAKEUP
n=0
for t in e8d616ba4 355b65140 e8d616ba4 355b65140; do
  n=$((n+1))
  (cd "w-$t/ts/app/web" && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
  node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:5174/');if(r.ok){return}}catch{}await new Promise(r=>setTimeout(r,1000))}process.exit(1)};go()"
  LABEL="s$n-$t" VPORT=5174 WT="w-$t" WAIT=120 PORT=9333 node pair.js s23 > "pair$n.log" 2>&1
  echo "run $n $t exit $?"
  P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P
  $A reverse --remove-all; $A forward --remove-all
done
python3 -I pairshow2.py s1-e8d616ba4 s2-355b65140 s3-e8d616ba4 s4-355b65140
python3 -I - <<'PY'
from PIL import Image,ImageChops
order=['s1-e8d616ba4','s2-355b65140','s3-e8d616ba4','s4-355b65140']
def st(a,b):
    A,B=[Image.open(x).convert('RGB') for x in(a,b)];W,H=A.size;box=(0,int(H*.13),W,int(H*.85))
    d=ImageChops.difference(A.crop(box),B.crop(box)).convert('L');h=d.histogram();t=sum(h)
    return 'mean %.2f over8 %.3f%% over32 %.3f%%'%(sum(i*c for i,c in enumerate(h))/t,sum(h[9:])/t*100,sum(h[33:])/t*100)
for a,b in(('s1-e8d616ba4','s3-e8d616ba4'),('s2-355b65140','s4-355b65140'),('s1-e8d616ba4','s2-355b65140'),('s3-e8d616ba4','s4-355b65140')):
    print('rest',a,b,st(f'pair-{a}-rest.png',f'pair-{b}-rest.png'))
A=Image.open('pair-s1-e8d616ba4-rest.png').convert('RGB');B=Image.open('pair-s2-355b65140-rest.png').convert('RGB')
ImageChops.difference(A,B).point(lambda v:min(255,v*8)).save('pair-s-diff-rest.png')
W,H=A.size;h=int(270*H/W);fs=[f'pair-{L}-{k}.png' for k in('rest','stop') for L in order]
c=Image.new('RGB',(4*270+30,2*h+10),(255,255,255))
for i,f in enumerate(fs):
    c.paste(Image.open(f).convert('RGB').resize((270,h)),((i%4)*280,(i//4)*(h+10)))
c.save('pair-s-all.png')
PY

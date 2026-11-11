#!/bin/zsh
A=~/Library/Android/sdk/platform-tools/adb
(cd wt/ts/app/web && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
(cd wt2/ts/app/web && ./node_modules/.bin/vite --port 5176 --strictPort >/dev/null 2>&1 &)
for p in 5174 5176; do node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:$p/');if(r.ok){console.log('up $p');return}}catch{}await new Promise(r=>setTimeout(r,1000))}console.log('not up');process.exit(1)};go()"; done
fr(){ VPORT=$2 WT=$3 MODE=frames COUNTS=0 SECS=25 WAIT=120 PORT=9333 node left2.js s23 $1 > frames-$1.log 2>&1; echo $1 $? $(date +%H:%M); }
fr offparent1 5176 wt2; fr offchild1 5174 wt; fr offparent2 5176 wt2; fr offchild2 5174 wt
python3 -I offshow.py offparent1 offchild1 offparent2 offchild2 | cut -c1-200
P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P
P=$(lsof -nP -tiTCP:5176 -sTCP:LISTEN); [ -n "$P" ] && kill $P
$A reverse --remove-all; $A forward --remove-all

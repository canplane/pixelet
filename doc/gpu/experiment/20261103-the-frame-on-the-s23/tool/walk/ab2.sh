#!/bin/zsh
(cd wt/ts/app/web && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
(cd wt2/ts/app/web && ./node_modules/.bin/vite --port 5176 --strictPort >/dev/null 2>&1 &)
for p in 5174 5176; do node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:$p/');if(r.ok){console.log('up $p');return}}catch{}await new Promise(r=>setTimeout(r,1000))}console.log('not up');process.exit(1)};go()"; done
A=~/Library/Android/sdk/platform-tools/adb
for i in 1 2 3; do
 VPORT=5176 WT=wt2 OUT=ab-parent-$i.json TURNS=6 WAIT=240 PORT=9333 node run6.js s23 > ab-parent-$i.log 2>&1; echo parent $i $?; $A reverse --remove-all
 VPORT=5174 WT=wt OUT=ab-child-$i.json TURNS=6 WAIT=240 PORT=9333 node run6.js s23 > ab-child-$i.log 2>&1; echo child $i $?; $A reverse --remove-all
done
for n in a b; do
 VPORT=5176 WT=wt2 SHOT=1 STILL=70 WAIT=240 PORT=9333 node pyramid.js s23 parent$n > shot-parent$n.log 2>&1; echo shot parent$n $?; $A reverse --remove-all
 VPORT=5174 WT=wt SHOT=1 STILL=70 WAIT=240 PORT=9333 node pyramid.js s23 child$n > shot-child$n.log 2>&1; echo shot child$n $?; $A reverse --remove-all
done
for p in 5174 5176; do P=$(lsof -nP -tiTCP:$p -sTCP:LISTEN); [ -n "$P" ] && kill $P; done

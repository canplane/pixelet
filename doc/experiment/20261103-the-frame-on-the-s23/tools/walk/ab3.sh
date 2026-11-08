#!/bin/zsh
A=~/Library/Android/sdk/platform-tools/adb
node -e "setTimeout(()=>{},300000)"
$A shell dumpsys thermalservice | grep -m1 'Thermal Status'
(cd wt/ts/app/web && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
(cd wt2/ts/app/web && ./node_modules/.bin/vite --port 5176 --strictPort >/dev/null 2>&1 &)
for p in 5174 5176; do node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:$p/');if(r.ok){console.log('up $p');return}}catch{}await new Promise(r=>setTimeout(r,1000))}console.log('not up');process.exit(1)};go()"; done
try(){ for t in 1 2 3; do "$@" && return 0; echo "retry $t: $1"; $A reverse --remove-all; $A forward --remove-all; done; return 1 }
fr(){ VPORT=$2 WT=$3 MODE=frames COUNTS=1 SECS=40 WAIT=90 PORT=9333 node left2.mjs s23 $1 > frames-$1.log 2>&1; }
sh(){ VPORT=$2 WT=$3 SHOT=1 STILL=70 WAIT=90 PORT=9333 node pyramid.mjs s23 $1 > shot-$1.log 2>&1; }
try fr fparent1 5176 wt2; echo f1 $?; try fr fchild1 5174 wt; echo f2 $?; try fr fparent2 5176 wt2; echo f3 $?; try fr fchild2 5174 wt; echo f4 $?
try sh parenta 5176 wt2; echo s1 $?; try sh childa 5174 wt; echo s2 $?; try sh parentb 5176 wt2; echo s3 $?; try sh childb 5174 wt; echo s4 $?
for p in 5174 5176; do P=$(lsof -nP -tiTCP:$p -sTCP:LISTEN); [ -n "$P" ] && kill $P; done
$A reverse --remove-all; $A forward --remove-all
$A shell dumpsys thermalservice | grep -m1 'Thermal Status'

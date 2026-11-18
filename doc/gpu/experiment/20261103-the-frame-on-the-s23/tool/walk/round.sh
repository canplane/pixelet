#!/bin/zsh
# round.sh <commit> <label> <runs perf?>: a commit's page on 5174, its windows and its pictures on the S23.
S=$PWD; C=$1; L=$2; PERF=$3
git -C wt checkout --quiet --detach $C || exit 9
export CARGO_TARGET_DIR="~/git/canplane/pixelet/rust/target/session/572d46ae-efea-42fb-a49d-2e5909f1eb71"
(cd wt/ts/app/web && pnpm build:wasm 2>&1 | tail -1 | cut -c1-70 && pnpm build:brand >/dev/null 2>&1)
(cd wt/ts/app/web && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &) 
node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:5174/');if(r.ok){console.log('vite up');return}}catch{}await new Promise(r=>setTimeout(r,1000))}console.log('vite not up');process.exit(1)};go()"
if [ "$PERF" = "1" ]; then TURNS=6,6 WAIT=180 PORT=9333 node run6.js s23 > run-$L.log 2>&1; echo perf $?; cp run4-s23.json run-$L.json; fi
for n in a b; do SHOT=1 STILL=70 WAIT=240 PORT=9333 node pyramid.js s23 $L$n > shot-$L$n.log 2>&1; echo shot $L$n $?; done
P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P

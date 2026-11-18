#!/bin/zsh
S=<scratch>
R=~/git/canplane/pixelet
A=~/Library/Android/sdk/platform-tools/adb
export CARGO_TARGET_DIR="$R/rust/target/session/572d46ae-efea-42fb-a49d-2e5909f1eb71"
cd $R
for t in e8d616ba4 355b65140; do
  git worktree add --detach "$S/w-$t" $t >/dev/null 2>&1 || { echo "worktree $t failed"; exit 1; }
  for p in $(find ts xpute/ts -name node_modules -type d -prune 2>/dev/null) xpute/ts/pnpm-lock.yaml; do
    [ -e "$p" ] && [ -d "$S/w-$t/${p%/*}" ] && [ ! -e "$S/w-$t/$p" ] && cp -c -R "$p" "$S/w-$t/$p"
  done
  (cd "$S/w-$t/ts/app/web" && pnpm build:wasm 2>&1 | tail -1 | cut -c1-60; pnpm build:brand >/dev/null 2>&1)
done
cd "$S"
$A shell input keyevent KEYCODE_WAKEUP
rm -f load-cold-*.json
for t in e8d616ba4 355b65140; do
  (cd "w-$t/ts/app/web" && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
  node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:5174/');if(r.ok){return}}catch{}await new Promise(r=>setTimeout(r,1000))}process.exit(1)};go()"
  VPORT=5174 CAP=150 node load2.js "cold-$t" cold 2>&1 | grep -E '^(errs|ok|clear)|rror' | cut -c1-900
  echo "cold $t done"
  P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P
  $A reverse --remove-all; $A forward --remove-all
done
python3 -I loadshow.py load-cold-e8d616ba4-cold.json load-cold-355b65140-cold.json | cut -c1-240 | grep -vE '^   lines|long tasks'

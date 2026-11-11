#!/bin/zsh
S=/private/tmp/claude-501/-Users-sanghoon-git-canplane-pixelet/572d46ae-efea-42fb-a49d-2e5909f1eb71/scratchpad
R=/Users/sanghoon/git/canplane/pixelet
A=~/Library/Android/sdk/platform-tools/adb
export CARGO_TARGET_DIR="$R/rust/target/session/572d46ae-efea-42fb-a49d-2e5909f1eb71"
cd $R
for t in f9f66682e e8d616ba4; do
  git worktree add --detach "$S/w-$t" $t >/dev/null 2>&1 || { echo "worktree $t failed"; exit 1; }
  for p in $(find ts xpute/ts -name node_modules -type d -prune 2>/dev/null) xpute/ts/pnpm-lock.yaml; do
    [ -e "$p" ] && [ -d "$S/w-$t/${p%/*}" ] && [ ! -e "$S/w-$t/$p" ] && cp -c -R "$p" "$S/w-$t/$p"
  done
  (cd "$S/w-$t/ts/app/web" && pnpm build:wasm 2>&1 | tail -1 | cut -c1-60; pnpm build:brand >/dev/null 2>&1)
done
cd "$S"
$A shell input keyevent KEYCODE_WAKEUP
n=0
for t in f9f66682e e8d616ba4 f9f66682e e8d616ba4; do
  n=$((n+1))
  (cd "w-$t/ts/app/web" && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
  node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:5174/');if(r.ok){return}}catch{}await new Promise(r=>setTimeout(r,1000))}process.exit(1)};go()"
  LABEL="$n-$t" VPORT=5174 WT="w-$t" WAIT=120 PORT=9333 node pair.js s23 > "pair$n.log" 2>&1
  echo "run $n $t exit $?"
  P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P
  $A reverse --remove-all; $A forward --remove-all
done
cd $R
for t in f9f66682e e8d616ba4; do git worktree remove --force "$S/w-$t"; done
git worktree prune
cd "$S"
python3 -I pairshow.py

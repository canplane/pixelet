#!/bin/zsh
S=<scratch>
R=$(git -C ${0:A:h} rev-parse --show-toplevel)
A=~/Library/Android/sdk/platform-tools/adb
cd "$S"
$A shell input keyevent KEYCODE_WAKEUP
n=4
for t in e8d616ba4 355b65140; do
  n=$((n+1))
  (cd "w-$t/ts/app/web" && ./node_modules/.bin/vite --port 5174 --strictPort >/dev/null 2>&1 &)
  node -e "const go=async()=>{for(let i=0;i<60;i++){try{const r=await fetch('http://localhost:5174/');if(r.ok){return}}catch{}await new Promise(r=>setTimeout(r,1000))}process.exit(1)};go()"
  LABEL="s$n-$t" VPORT=5174 WT="w-$t" WAIT=120 PORT=9333 node pairoff.js s23 > "pair$n.log" 2>&1
  echo "run $n $t exit $?"
  P=$(lsof -nP -tiTCP:5174 -sTCP:LISTEN); [ -n "$P" ] && kill $P
  $A reverse --remove-all; $A forward --remove-all
done
python3 -I pairshow2.py s5-e8d616ba4 s6-355b65140
cd $R
for t in e8d616ba4 355b65140; do git worktree remove --force "$S/w-$t"; done
git worktree prune
git worktree list | grep -c scratchpad

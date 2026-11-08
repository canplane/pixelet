#!/bin/zsh
# ./ab.sh NAME ROUNDS PORT:TREE PORT:TREE … : s23.js on each page by turns, ROUNDS times, a fresh load
# each, labelled NAME<n>-<port>. Before every load the port's server must be a Metro whose cwd is
# TREE/ts/client, or nothing runs: a stale server on a port has measured the wrong tree before.
# The env s23.js reads (SCENES, HOUR, SETTLE_MS, …) passes through.
cd "${0:A:h}"
N=$1; RN=$2; shift 2
cwd_of(){ local P=$(lsof -nP -tiTCP:$1 -sTCP:LISTEN | head -1); [ -n "$P" ] && lsof -a -p $P -d cwd | tail -1 | awk '{print $NF}'; }
# Each sitting begins cool: thermal status 0 and the AP under 38 °C.
cool(){ local A=~/Library/Android/sdk/platform-tools/adb; node -e "
const {execSync}=require('child_process');(async()=>{for(;;){const o=execSync('$A shell dumpsys thermalservice').toString();const s=+(o.match(/Thermal Status: (\\d+)/)||[0,9])[1];const t=+(o.match(/mValue=([0-9.]+), mType=0, mName=AP/)||[0,99])[1];if(s===0&&t<38){console.log('cool',t);return}await new Promise(r=>setTimeout(r,20000))}})()"; }
cool
n=0
for r in $(seq $RN); do
  for pt in "$@"; do
    p=${pt%%:*}; t=${pt#*:}
    if [ "$(cwd_of $p)" != "$t/ts/client" ]; then echo "port $p is not $t (it is '$(cwd_of $p)'): stopped"; exit 1; fi
    n=$((n+1)); L=$N$n-$p
    VPORT=$p LABEL=$L node s23.js > /dev/null 2>&1
    echo "$L exit $?"
  done
done

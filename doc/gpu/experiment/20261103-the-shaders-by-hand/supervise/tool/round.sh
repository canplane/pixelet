#!/bin/zsh
# Fresh copy of the tree, the implementer's test run as it stands, the mutant loop added on the copy.
SP=/private/tmp/claude-501/-Users-sanghoon-git-tmaxoffice-cx/eff52c1d-b098-47ac-a575-b58a0e234362/scratchpad
R=/Users/sanghoon/git/canplane/pixelet
rsync -a --delete --exclude target --exclude node_modules --exclude .git --include='rust/***' --include='xpute/' --include='xpute/rust/***' --exclude='*' $R/ $SP/copy/
python3 $SP/tool/patch.py $SP/copy/rust/client/render/tests/world.rs
python3 - <<PY
p='$SP/copy/rust/client/render/tests/world.rs';s=open(p).read()
s=s.replace('let parts: Vec<&str> = line.split(" ||| ").collect();','let line = line.replace("\\\\\\\\n", "\\\\n");\n            let parts: Vec<&str> = line.split(" ||| ").collect();')
open(p,'w').write(s)
PY
cd $SP/copy/rust && CARGO_TARGET_DIR=$SP/target cargo test -p pxl-render --test world --no-run 2>&1 | grep -E "^error|Finished" -A6 | head -20

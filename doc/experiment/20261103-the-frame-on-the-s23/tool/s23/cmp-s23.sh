#!/bin/zsh
M=/Users/sanghoon/git/canplane/pixelet; O=$M/cache/cmp; export OUT=$O/s23
mkdir -p $OUT; cd $O
node hot.mjs 8298 warm 100 2>&1 | tail -1
for v in "8298 main-1" "8291 base-1" "8291 base-2" "8298 main-2"; do REST=65 node hot.mjs ${=v} 120 2>&1 | tail -1; done

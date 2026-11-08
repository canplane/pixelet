#!/bin/zsh
M=/Users/sanghoon/git/canplane/pixelet; O=$M/out/cmp
(cd $M/ts/client && CARGO_TARGET_DIR=$O/t-main pnpm build:wasm && CARGO_TARGET_DIR=$O/t-main pnpm build:brand && node script/web.ts html) > $O/main-build.log 2>&1 || { tail -5 $O/main-build.log; exit 1; }
lsof -nP -iTCP:3001 -sTCP:LISTEN >/dev/null || (cd $M/rust && CARGO_TARGET_DIR=$O/t-main nohup ../script/with-env.sh cargo serve-tile --shelf $M/out > $O/tile.log 2>&1 &)
lsof -nP -iTCP:8298 -sTCP:LISTEN >/dev/null || (cd $M/ts/client && PXL_DEV_SOURCE=shelf EXPO_NO_TYPESCRIPT_SETUP=1 CI=1 nohup ./node_modules/.bin/expo start --web --port 8298 > $O/main.log 2>&1 &)
for u in 127.0.0.1:3001/tile/source localhost:8298/; do curl -s -o /dev/null -m 200 --retry 120 --retry-delay 2 --retry-connrefused -w "$u %{http_code}\n" $u; done
curl -s -o /dev/null -m 200 -w 'bundle %{http_code} %{size_download}\n' 'http://localhost:8298/client/index.ts.bundle?platform=web&dev=true&hot=false&lazy=true&transform.engine=hermes&transform.routerRoot=app&unstable_transformProfile=hermes-stable'
grep -c 'build-wasm' $O/main-build.log; grep build-wasm $O/main-build.log | tail -1 | cut -c1-90

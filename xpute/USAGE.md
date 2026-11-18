# Using xpute

This is how to build a program on xpute, from nothing to a call that crosses:
a guest in Rust that adds two numbers, and a web page that asks it to. What
xpute is, and why it is shaped so, is in [README.md](./README.md).

## What it takes

- Node at the version `.tool-versions` names, with pnpm.
- Rust at the version `rust-toolchain.toml` names or later, by rustup, with
  rustfmt and the target `wasm32-unknown-unknown`.
- xpute's tree beside the program's, and its generator installed from it,
  from the directory that holds both:

  ```sh
  cargo install --path xpute/link/gen
  ```

The program here is three directories under one root: `wit/`, its interfaces;
`guest/`, a Rust crate; and `ui/`, a TypeScript package.

## 1. Lay out the crate and the package

The generator names what it writes by the manifest above it, so both come
first. `guest/Cargo.toml` names xpute's three crates, which the bindings use:

```toml
[package]
name = "demo-guest"
edition = "2024"

[dependencies]
xpute-guest = { path = "../../xpute/guest/rust" }
xpute-link = { path = "../../xpute/link/rust" }
xpute-kit = { path = "../../xpute/kit/rust" }
```

`ui/package.json` depends on xpute's packages:

```json
{
  "name": "demo-ui",
  "type": "module",
  "dependencies": {
    "@xpute/host": "workspace:*",
    "@xpute/kit": "workspace:*",
    "@xpute/link": "workspace:*",
    "@xpute/runtime-web": "workspace:*"
  }
}
```

The root's `package.json` brings prettier, with which the generator formats
the TypeScript it writes:

```json
{
  "private": true,
  "devDependencies": {
    "prettier": "^3"
  }
}
```

And the root's `pnpm-workspace.yaml` lists the package with xpute's, the
reader of the tests' records among them, which one of them depends on:

```yaml
packages:
  - ui
  - ../xpute/kit/ts
  - ../xpute/link/ts
  - ../xpute/host/ts
  - ../xpute/runtime/web
  - ../xpute/golden/ts
```

`pnpm install`, in the root, links them.

## 2. Declare the interfaces

One package, in `wit/app.wit`. Its world says what the guest implements, its
exports, and what it calls, its imports. Every guest imports xpute's stop, by
which it reports its own bug:

```wit
package demo:app@0.1.0;

interface calc {
  add: func(a: u32, b: u32) -> u32;
}

world guest {
  import xpute:stop/stop@0.1.0;
  export calc;
}
```

## 3. Write the bindings

From the program's root, once for each side. Run them again whenever the WIT
changes:

```sh
xpute-link-gen guest wit --rs guest/src/abi.rs
xpute-link-gen host wit --ts ui/src/abi
```

The first writes `guest/src/abi.rs` and a module for each interface beside it.
The second writes a module for each interface under `ui/src/abi/`, with
`imports.ts` and `calls.ts`, the lists the runtime starts the guest from.

## 4. Write the guest

`guest/src/lib.rs` implements the trait the bindings give it, and mounts the
implementation in its boot, the second argument of `link!`:

```rust
mod abi;

use std::rc::Rc;

use abi::calc;
use xpute_kit::status::errno::Errno;

struct App;

impl calc::Guest for App {
    fn add(&self, a: u32, b: u32) -> Result<u32, Errno> {
        Ok(a.wrapping_add(b))
    }
}

xpute_guest::link!(4096, || calc::mount(Rc::new(App)), || {}, || false);
```

Build it as a module, in `guest/`:

```sh
cargo rustc --crate-type cdylib --release --target wasm32-unknown-unknown
```

The module is `target/wasm32-unknown-unknown/release/demo_guest.wasm`. Put it
where the page's server serves files.

## 5. Write the worker and the page

The guest runs in a worker. `ui/src/worker.ts` starts it, with xpute's
implementation of the stop mounted:

```ts
import { mount_stop } from "@xpute/host/stop.ts";
import { type Instantiate } from "@xpute/runtime-web/message.ts";
import { instantiate, run } from "@xpute/runtime-web/worker.ts";
import { CALLS } from "./abi/calls.ts";
import { IMPORTS } from "./abi/imports.ts";

onmessage = async ({ data: m }: MessageEvent<Instantiate>) => {
  const guest = await instantiate(m.module, m.init);
  if (guest === null) return;
  run(
    guest,
    {
      mounts: [mount_stop],
      imports: IMPORTS,
      calls: CALLS,
      taker: { give() {}, take() {} },
    },
    m,
  );
};
```

The page opens its end, starts the guest from the module's URL, and calls it
by the function the bindings wrote (`ui/src/page.ts`):

```ts
import { sender } from "@xpute/link/wait.ts";
import { open_ui, start, stopped_on, trap_on } from "@xpute/runtime-web/page.ts";
import { add } from "./abi/calc.ts";

const ui = open_ui(new URL("./worker.ts", import.meta.url));
stopped_on((stop) => console.error(stop));
trap_on((err) => console.error(err));
start("/demo_guest.wasm", undefined);
console.log(await add(sender(ui), 2, 3)); // 5
```

xpute's packages are TypeScript sources, so the page and the worker go through
the program's bundler, one that takes TypeScript and a module worker; how the
worker's URL is written is that bundler's.

## When it does not start

- `trap_on` hears `no implementation of …`: the world imports an interface
  nothing in `mounts` mounts.
- `trap_on` hears a failed fetch or instantiation: the URL given to `start`
  does not serve the module.
- `stopped_on` hears a stop: the guest panicked or met a bug, or the worker's
  own code threw, and the record says where. A trap follows it.

## From here

- More of WIT, a method answered later (`async func`), an object opened by a
  URI, a buffer lent in place: [link/gen](./link/gen/README.md).
- What the guest's entries are given, and its memory:
  [guest/rust](./guest/rust/README.md); a guest in C++: [cpp](./cpp/README.md).
- An interface the UI implements, and a thing handed beside a call:
  [runtime/web](./runtime/web/README.md).
- The same guest in an iOS or Android app: the crate that starts it
  ([runtime/native](./runtime/native/README.md)) and the Expo module that
  builds and links it ([expo](./expo/README.md)).

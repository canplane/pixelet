# Third-party notices

Two places in this repository are ports of someone else's code rather than
our own, and their licences ask for a notice to travel with the copy. One
carries it inside the file; the other cannot, so it is reproduced here.

This file covers copied source only. Crates and npm packages that are merely
depended on are not listed: `crates/Cargo.toml` and `apps/web/deno.jsonc`
name them, and each carries its own licence where it is published.

## shadcn/ui — `apps/web/src/shared/ui/shadcn/`

The components under that directory came from [shadcn/ui][shadcn] and are
edited in place, which is how that project is meant to be used. It is a flat,
uniformly header-less tree — AGENTS.md says not to add headers to it — so the
notice its licence asks for lives here instead of in six files.

[shadcn]: https://github.com/shadcn-ui/ui

```
MIT License

Copyright (c) 2023 shadcn

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## evanw/buddy-malloc — `crates/xpute/core/src/alloc/buddy_tree.rs`

That file is [evanw/buddy-malloc][buddy] (MIT) line for line, and the notice
is at the top of it.

[buddy]: https://github.com/evanw/buddy-malloc

Data sources have their own obligations and their own place:
[infra/README.md](./infra/README.md) §6 records them, and
`apps/web/src/feature/setting/section/attribution.tsx` is where the ones
that must reach a user are shown.

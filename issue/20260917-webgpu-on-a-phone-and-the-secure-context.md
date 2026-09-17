# WebGPU on a phone, and the secure context in the way

**2026-09-17** — The dev server drew nothing and said so:

```
[gpu] no WebGPU on this device; nothing can be drawn
```

The device had WebGPU. The origin did not. The page was open at
`http://192.168.0.2:5173` rather than `http://localhost:5173`, and
`navigator.gpu` is not exposed outside a secure context — same Chrome, same
machine, same dev server, two addresses, one of which can draw.

This records what the requirement actually is, why it arrived now rather than
earlier, why an iPhone is the hard case, and what the ways around it cost.
Nothing here is wired; only the message was fixed.

## The requirement is the origin, not the device

WebGPU's IDL puts `gpu` on `Navigator` behind `[SecureContext]`, so on an
origin the browser does not consider potentially trustworthy the attribute is
simply absent — there is no adapter to refuse, no error to catch, nothing to
feature-detect past `"gpu" in navigator`. The Secure Contexts rules treat
`http://localhost` and `http://127.0.0.1` as trustworthy and a private LAN
address as not; `192.168.0.2` is an ordinary insecure origin as far as the
browser is concerned.

`apps/web/vite.config.ts` sets `host: true`, so the dev server binds every
interface and both addresses reach the same process. Which one is in the
address bar decides whether the client can draw, and nothing in the page says
so.

## The message named the wrong cause

`acquire_device` returned `null` for three different situations — no
`navigator.gpu`, no adapter, and (by rejecting) no device — and the shell
printed one sentence for all three. The sentence it printed blamed the device,
which is the one of the three that was not true here, and it sent the reading
in the wrong direction for as long as it was believed.

It now returns a reason and the caller prints it, so the three read apart:
whether the origin is the problem, whether the browser has no WebGPU at all,
and whether an adapter declined. The rejection path had no handler at all and
escaped as an unhandled rejection with no `[gpu]` line anywhere; it has one
now. This matters on the re-boot after a device loss, which is the only place
`requestDevice` is called a second time.

## Why this is new

Before 2026-09-14 the client drew through WebGL, which carries no
secure-context requirement, so an insecure LAN origin was a perfectly good
place to run the whole client. The renderer moving into the module is what put
a `[SecureContext]` attribute on the critical path.

That makes one existing record wrong as written:
[`experiment/20260913-bake-worker-on-the-phone.md`](./experiment/20260913-bake-worker-on-the-phone.md)
describes its procedure as an iPhone reaching the dev server over the LAN,
which was fine for what it measured — a bake worker needs no GPU — and is no
longer a procedure that produces a picture. Anyone following it now gets a
blank canvas and the message above.

## Why an iPhone is the hard case

Android Chrome has two ways out and neither needs a certificate:

- `chrome://inspect` → Port forwarding maps the desktop's `localhost:5173` onto
  the phone over USB, so the phone's own origin is `http://localhost:5173`,
  which is trustworthy by the same rule that makes it work on the desktop.
- `chrome://flags/#unsafely-treat-insecure-origin-as-secure` takes an explicit
  origin and treats it as trustworthy, per browser.

iOS has neither. Every browser on it is WebKit, there is no flag surface for
this, and there is no port-forwarding path. The only door is a real `https://`
origin, which means a certificate the phone will accept.

## What the ways cost

None of `mkcert`, `cloudflared`, `ngrok`, `tailscale` or `caddy` is installed
on this machine, so every row below starts from an install.

| Way                        | Installs                                       | Phone-side work                                                                                                        | What it costs                                                                                                            |
| -------------------------- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `@vitejs/plugin-basic-ssl` | a Vite plugin, no system tool                  | accept the warning once per session                                                                                    | self-signed, so the interstitial returns; HMR rides the same accepted origin                                             |
| `mkcert`                   | a local CA on this machine                     | AirDrop the root CA, install the profile, then enable it under Settings › General › About › Certificate Trust Settings | several one-time steps, after which there are no warnings at all                                                         |
| Tailscale                  | the app here and on the phone, plus an account | join the tailnet                                                                                                       | a real Let's Encrypt certificate for the tailnet name, no CA to install, and a direct WireGuard path rather than a relay |
| a deploy to `plei.me`      | nothing                                        | nothing                                                                                                                | no setup, but the loop is a deploy and what runs is the built bundle, not the dev server                                 |

The first is the cheapest thing that works: once the interstitial is passed the
origin is `https://`, and a secure context is a secure context whether or not
the certificate was trusted.

## Two things that bite either way

- **A tunnel needs `server.allowedHosts`.** Vite 7 rejects a `Host` header it
  does not recognize, and a `*.trycloudflare.com` or `*.ngrok-free.app` name is
  not one of them. `vite.config.ts` sets no `allowedHosts` today. The
  certificate-based rows above are addressed by IP or by a tailnet name and are
  not affected.
- **Seeing the console at all is separate setup.** iPhone Settings › Safari ›
  Advanced › Web Inspector, plus the Develop menu in Safari on this machine.
  Without it a diagnosis like this one is invisible on the device it happens on.

WebGPU itself is not the obstacle on the floor device: Safari 26 on iOS 26 has
it on by default, and iOS 26 is the floor
([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md)).

## Where it stands

**Open.** The message is fixed and the diagnosis is recorded; no https path is
wired. The desktop loop is unaffected — `http://localhost:5173` draws — so this
is only in the way of testing on a phone, which the floor device makes a thing
that has to work eventually rather than a convenience.

Also noticed while reading and not acted on: `gpu_boot`'s `lost` callback calls
`gpu_boot` again unconditionally, so a device that is lost and cannot be
reacquired retries without end and without a second message.

# The transport that is already there, and the one that is not

**2026-09-20** — a design conversation, not a code change. The question was
whether to lay HTTP/3 and WebTransport as a conduit now, with a fallback for
where it cannot connect, and let `pxl-api`'s routes accumulate through it
afterwards. That is the same move `pxl-api` itself already made — it answers
nothing and exists anyway, because the shape is what a later route arrives
into — so the instinct is the project's own and not a departure from it.

Nothing was installed. What follows is what the measurement found, because
the answer turned on two facts neither side of the question assumed: one of
the two transports is already compiled into the binary, and the other is
blocked by the deployment's shape rather than by a missing library.

## Only one plane is in question

The baked half is not in this picture and must not be drawn into it.
Terrain and asset objects are immutable prefixes served at
`max-age=31536000, immutable`, POI and geocode at a day; the Worker holds
them in `caches.default`, the bucket is private so every read goes through
the Worker, and that composition is also what keeps the deployment unable
to bill ([20260816-deployment-hosting.md](./20260816-deployment-hosting.md)).
Moving any of it onto a stream would give up the edge cache, the browser
cache and that guarantee together.

So the question reaches only the mutable, bidirectional half — commands,
chat, presence — and the two planes stay two. BACKLOG already draws the
same line from the other direction: the codec stays in the transport, never
in the command frame.

## HTTP/3 is not an agenda item

The client-to-edge leg speaks HTTP/3 because Cloudflare is in front of it,
and the edge-to-origin leg is Cloudflare's business. Nothing in the repo
decides either. Server-side HTTP/3 matters here only as the substrate
WebTransport needs, which makes this one question and not two.

## WebSocket is already in the binary

`ws` is a **default** feature of axum 0.8, so `axum::extract::ws` is
compiled into `pxl-api` today. Caddy proxies an Upgrade like any reverse
proxy, and Cloudflare states WebSocket support on all plans with no
configuration. The path is open end to end and nothing has walked it.

What is missing is therefore not a dependency. It is a round trip nobody
has taken, and a test that would notice if the path closed.

## WebTransport: the browser is nearly solved, the topology is not

Safari 26.4 shipped WebTransport in March 2026 and it reached Baseline in
April. The floor is iOS 26, so only 26.0–26.3 needs the fallback — a
shrinking minority rather than the majority the fallback would have been a
year ago.

The gate is the deployment, and each piece of it is deliberate:

- `api.plei.me` is an A record to the VM **with Cloudflare's proxy on**,
  and 80/443 are firewalled to Cloudflare's ranges. `infra/README.md` says
  why in its own words: DNS-only skips the edge's DDoS mitigation and makes
  that firewall pointless.
- Cloudflare's proxy carries WebSocket by documented policy and says
  nothing about WebTransport; the reports that exist are of it not arriving
  through Tunnel and Load Balancing.
- Caddy terminates TLS over TCP and relays HTTP. WebTransport is not
  request/response — it is an Extended CONNECT session over QUIC — so
  there is nothing for it to relay.
- The `api` container publishes no host port at all, on purpose.

QUIC is UDP/443. Reaching the container means changing all four.

The crate is not one line either. `wtransport 0.7.2` over quinn, or
`h3 0.0.8` with `h3-webtransport 0.1.2` and `quinn 0.11.12` — the latter
still on 0.0.x versioning. Either brings a second TLS stack and a second
listener, and neither composes with the axum `Router`, because an Extended
CONNECT session is not a request.

So installing it now buys a dependency that cannot be exercised against the
deployed shape: loopback would prove the crate links, not that the
deployment can carry it. A library kept ready for later does not help when
what blocks later is the topology.

## The frame already reserved the room

`ipc/frame.rs` states the separation the whole question turns on: every
boundary carries the same four words, and a transport says only where the
words sit and how word 3 names the packet. Two reservations are already
spoken for:

- **`tag`'s high bits** — for exactly this. Put a switch between the two
  sides, several workers _or a server_, and a reply has to find its way
  back without a lookup.
- **`FrameFlag`'s bits 2 and up** — what the low half of word 1 means.

April built that switch and routed by the high bits of `tx_id`; what was
judged worth dropping was the signal plane, the selector and the transports
([20260913-boundaries-commands-and-what-comes-back-from-april.md](./20260913-boundaries-commands-and-what-comes-back-from-april.md)),
never the frame. So the work, when it comes, is not "adopt WebTransport".
It is one layer that writes the four words and their XTP packet as bytes:
words 0–2 unchanged, word 3 changing only what it names — an address in the
ring, a length and the bytes after it on a wire.

That layer is a protocol, so it is held to bytes and carries golden records
([20260920-inside-a-language-and-across-a-protocol.md](./20260920-inside-a-language-and-across-a-protocol.md)).

## Where this sits in the order

[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
fixes the dependency: storage model, resource identity, user content,
command unification, multiplayer. Steps 1 and 2 were paid for on 09-13 and
3 is decided and unbuilt, so the April warning it records — that fixing a
command interface freezes a carrier around a payload whose shape is not
decided — has expired for the storage model.

It has not expired for the session. Nothing yet decides what identifies a
sender, and that is precisely what `tag`'s high bits would carry. The
command-channel document
([20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md))
left it open in the same words, along with its assumption that the
transport would be a websocket — which is the assumption this document
measures rather than replaces.

One reading is worth correcting while here. AGENTS.md puts "traffic/scale
optimization (AOI filtering, decay systems, sync protocols)" outside MVP
scope. That excludes optimising sync at scale; it does not exclude having a
channel, which is step 4 of the order and a feature rather than an
optimisation.

## Where WebTransport earns its keep, and where it does not

It earns it on unreliable datagrams — a position or a heading, where only
the newest value means anything and a lost one should not delay the next.
Every WebSocket message is reliable and ordered, so one that stalls stalls
everything behind it, and that is the cost that grows with the regime
[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
describes.

It does not earn it on commands. A placement, an erase or a chat line needs
reliability and arrives a few times a second; head-of-line blocking barely
shows on a single stream at that rate.

So it belongs to step 5 and not to step 4.

## The order if it is taken up

1. **Measure before buying.** One QUIC handshake at `api.plei.me` on
   UDP/443. If it does not arrive, no crate choice matters and the rest of
   this list is about infrastructure rather than code.
2. **Hold the WebSocket round trip with a test.** `tokio-tungstenite` as a
   dev-dependency, a listener on port 0, and **nothing mounted in the
   deployed router** — an echo endpoint on a live server is a
   connection-exhaustion surface, which is why `/api/internal/*` sits behind
   the admin token. The test builds its own router; the public surface stays
   zero and CI keeps proving the path.
3. **The frame's network encoding**, once there is a first command to carry
   and a session to name the sender.
4. **WebTransport as a second backend under that encoding**, never as a
   transport the commands know about. The WebSocket is the baseline and the
   upgrade is opportunistic, not the other way round: a path reached only
   when the preferred one fails is a path nobody walks, and therefore one
   nobody has tested.

## Status

Recorded, not scheduled. Nothing installed, nothing mounted, no dependency
added.

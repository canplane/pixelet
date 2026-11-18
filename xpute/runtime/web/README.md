# @xpute/runtime-web

@xpute/runtime-web is xpute's runtime on the web: the guest runs as a
WebAssembly module in a worker of its own, and the UI reaches it from the
page's main thread over a MessagePort. It has two halves, one for each
thread, which a program's own worker and page import: the worker's
(`worker.ts`) and the page's (`page.ts`). How the runtime carries a UI to the guest, and why every method
the UI implements is an `async func`, is in
[ARCHITECTURE.md](../../ARCHITECTURE.md), "The sides".

## The worker

The program's worker is sent one message as the page starts the guest
(`Instantiate`), and starts the guest from it in two steps. `instantiate`
fetches the guest's module and lays a link over it, answering null where the
module does not load, which the page hears as a trap. `run` then starts the
guest from the program's `Boot`, by the steps the ARCHITECTURE gives: its
mounts, each of which mounts on the link its implementations; the bindings'
`IMPORTS` and `CALLS`; and a `taker`, which takes what the UI gives beside its
calls under a handle, a canvas for one, which the web's port can move. Here the
program mounts a plugin of its own beside xpute's host:

```ts
// The worker, given the page's `Instantiate`, m.
const link = await instantiate(m.module, m.init);
if (link === null) return;
const view = new View(link.side); // a plugin of the program's
run(link, {
  mounts: [
    mount_stop,
    mount_clock,
    () => view.mount(),
  ],
  imports: IMPORTS,
  calls: CALLS,
  taker: {
    give: (h, canvas) => view.bind(h, canvas as OffscreenCanvas),
    take: (h) => view.unbind(h),
  },
}, m);
```

Each log record the worker writes is told the page too (`Log`), which writes
nothing of it again. The host reads the guest's memory through a view made at
each access, so a growth of the module's memory detaches nothing.

## The page

The UI meets the runtime as the native UI meets its own (`expo/README.md`): it
opens its end (`open_ui`), hears the guest's stop and its trap (`stopped_on`,
`trap_on`), mounts on its end what it implements, and starts the guest
(`start`). Two things differ by the platform. It names the worker's module as
it opens its end, and as it starts the guest, the guest's module, how to fetch
it and any fields of the program's own its worker reads beside them. And the
start tells the guest the page's lifecycle from the document's visibility,
which a worker cannot see. What the worker posts the page is a message by its
kind, the runtime's, a plugin's and the program's alike, which the UI hears by
its kind as it comes (`msg_on`); nothing is kept for a late hearer, since
the worker dies with its page. `give` hands an object beside the calls under a
handle the worker's end issued:

```ts
const ui = open_ui(WORKER_URL);
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
msg_on<ViewReport>("view", (m) => { /* show it */ });
// Mount on `ui` what the UI implements, before `start`.
start(MODULE_URL, undefined);
const view = await open(sender(ui), uri("app:view", "view"));
give(view, canvas.transferControlToOffscreen());
```

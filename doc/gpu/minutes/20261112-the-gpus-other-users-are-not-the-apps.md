# The GPU's other users are not the app's

**2026-11-12** — recorded with the user, in the session that chased the Mac Safari
freeze (`9a2d4066dc`). What was measured is in
[the Safari stall](../experiment/20261111-the-safari-stall/README.md), and what was
learned of it in [the GPU is shared](../lesson/the-gpu-is-shared.md).

## What the user decided

**The app does not detect another user of the GPU.** Telling from inside the page or the
app that something else is on the GPU is no different from code asking whether it runs on
Deno, Node or a browser. A stall that another program or the platform causes is the device
owner's to deal with, not the app's.

**No workaround for WebKit bug 321876** (what it is and how decor meets it are in the
lesson). A decor-only path around it would be ad hoc.

**No runs to show a stall gone.** Runs that did not stall say only that it has not
stalled so far.

**The repository's own tools hold the device they drive.** The agents' harness is the
repository's own, so its users do not share a device, and this holds alike on every
platform.

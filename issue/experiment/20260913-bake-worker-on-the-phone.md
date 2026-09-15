# The bake worker on the phone

**2026-09-13** — the A/B the worker's case was left to
([20260909-cooperative-scheduler-and-task-ledger.md](../20260909-cooperative-scheduler-and-task-ledger.md),
"The worker's case", corrected the same day). Both bake paths were built
behind the dev setting `bake_path` (d9dda7b): `stepped`, the bake cut into
steps on the main thread under the frame's budget, and `worker`, the bake
whole in a worker with the main thread copying the mesh in.

## Procedure

An iPhone, `deno task dev:remote` over the LAN (unbundled modules), a place
reached by search, one session per path, the ledger report taken at the
end of each. The two sessions were not the same scene: the worker session
drew 1.3x the triangles (decor cars alone 182k) and began with a fetch
queue wait of over seven seconds that the stepped session did not have.
The traces were exported as `pxl-ledger-2026-09-13T07-32-46-433Z.json`
(stepped) and `pxl-ledger-2026-09-13T07-34-04-897Z.json` (worker) and are
not in the repository.

The reports mix loading with settled motion, so the frames were split from
the traces: **filling** is `lod_pending > 0`, **settled** is nothing pending
and the camera moving. A frame is missed when its dt is over 1.5 display
frames.

## Numbers

Filling:

|                    | stepped | worker  |
| ------------------ | ------- | ------- |
| frames             | 97      | 277     |
| bakes finished     | **167** | 127     |
| missed             | **19%** | 41%     |
| task p50 / p95     | 9 / 27  | 9 / 26  |
| commit p95 / max   | 16 / 22 | 17 / 47 |
| scans (main, both) | 100     | 103     |

Settled, moving:

|               | stepped | worker |
| ------------- | ------- | ------ |
| missed        | 11%     | 30%    |
| task p95      | 9       | 12     |
| offthread p95 | 24      | 29     |
| triangles p50 | 352k    | 454k   |
| commit p95    | 1       | 1      |

From the reports: the stepped path's **longest single step was 6.0 ms**. The
worker's time away averaged 71 ms with a maximum of 2,321 ms; adopting a
mesh on the main thread averaged 0.2 ms (max 11). Throughput over each
window, 12.4 bakes/s stepped against 6.6 worker. No spills, no cap
deferrals, no failures.

## What it showed

- **The stepped bake is cooperative on the phone.** Its longest step fits
  the 8 ms budget. The 64–112 ms `visible/chunks` maxima the worker was
  first justified by were not bake steps.
- **The worker did not take cost off the main thread.** While filling, the
  task p95 is the same on both paths (27, 26 ms) with the same scans behind
  it. What the main thread spends while terrain fills in is not the bake.
- **The stepped path filled faster.** More bakes in a third of the frames,
  at the same main-thread cost. One worker serialises its bakes, the phone
  may schedule it on an efficiency core, and every bake pays two copies.
- **The settled gap is the scene, not the path.** Both settled windows
  baked 40 chunks with commit p95 at 1 ms; the worker session drew more and
  its off-thread time was higher. Nothing here attributes the missed frames
  to the worker.

## What became of it

`stepped` stays the default and the worker path stays behind the setting,
not deleted: it is small, tested, and the frame, port and link it stands on
are the boundary the later pieces use. Both paths and the setting were
retired on 2026-09-14 (`archive/20260914-ts-build-paths`), when the bake
became the module's. P0-1 is closed on these numbers, and
the Rust mesher no longer presumes a worker — where it runs is
measured when it exists.

Not measured:

- **Memory.** Safari exposes no JS heap to the ledger (`heap 0MB` on both).
  What it does show is close: the core pool live 232 MB stepped against
  241 MB worker, 320 MB reserved on both, no slab left away. The worker's
  own memory — its JavaScript realm and its 16 MB regions — is invisible to
  the ledger, and so is whatever WebKit keeps per upload. Neither session
  was watched for pressure events. A Safari heap snapshot and the memory
  timeline are the instruments, if the worker path is ever reconsidered.
- The same scene on both paths, and a production build.
- Where the filling window's commit time goes (scan, bake, attach, mount) —
  deferred on purpose; see BACKLOG.

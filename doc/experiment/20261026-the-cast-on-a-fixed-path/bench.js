// doc/experiment/20261026-the-cast-on-a-fixed-path/bench.js
//
// Pasted into a development page's console once the kernel has booted, with
// the `profiler` setting on and `resolution` fixed. Every move is a command
// through the guest's doorbell on a wall-clock schedule (cmd.json numbers
// them), so no hand touches the camera and each load walks the same path.
// Resolves to the profiler's report over the path alone.
(async () => {
  const WHEEL = 4355, POINTER = 4354, PROFILER_START = 6401;
  const DOWN = 0, MOVE = 1, UP = 2, MOUSE = 0, RIGHT = 2;
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // A wheel's travel in equal steps; each multiplies the distance by
  // exp(delta × DOLLY_GAIN), 0.0022 (pxl-kernel domain/control/param.rs).
  async function wheel(total, n, ms) {
    for (let i = 0; i < n; i++) {
      xpute.cmd(WHEEL, total / n);
      await sleep(ms / n);
    }
  }

  // A right drag a quarter round the canvas's center, from above to the
  // right: the azimuth follows the cursor's angle (camera/input.rs
  // `do_rotate`), which is not unwrapped, so no drag crosses ±180°.
  async function quarter(r, n, ms) {
    const cx = innerWidth / 2, cy = innerHeight / 2;
    const at = (a) => [cx + r * Math.cos(a), cy + r * Math.sin(a)];
    let [x, y] = at(-Math.PI / 2);
    xpute.cmd(POINTER, DOWN, MOUSE, 1, RIGHT, x, y);
    for (let i = 1; i <= n; i++) {
      [x, y] = at(-Math.PI / 2 + (Math.PI / 2) * (i / n));
      xpute.cmd(POINTER, MOVE, MOUSE, 1, RIGHT, x, y);
      await sleep(ms / n);
    }
    xpute.cmd(POINTER, UP, MOUSE, 1, RIGHT, x, y);
  }

  await sleep(5000);
  xpute.cmd(PROFILER_START);
  await wheel(2520, 30, 3000); // out: 16 → about 4096
  await sleep(2000);
  await wheel(-3300, 30, 3000); // in, to the floor of 2
  await wheel(945, 15, 1500); // back to 16
  for (let q = 0; q < 4; q++) await quarter(300, 25, 1000); // once round
  await sleep(3000);
  return pxl.profiler.report();
})();

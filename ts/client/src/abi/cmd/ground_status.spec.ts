// ts/client/src/abi/cmd/ground_status.spec.ts
//
// GENERATED from rust/client/kernel/shell.json

// Whether the screen shows the world: Success once a frame that drew ground went
// to it, or one went over a view whose tiles all answered with nothing to draw;
// Loading until then, on a first load, after a jump or a lost device; Idle before
// there is a view.

export const Idle = 0;
export const Loading = 1;
export const Success = 2;

// @pxl/web/shared/lib/store/wait_until.ts

export type Unsubscribe = () => void;
export type SubscribeWake = (wake: () => void) => Unsubscribe;

export function wait_until<T>(try_get: () => T | null, subscribe_wake: SubscribeWake, on_ready: (value: T) => void): Unsubscribe {
  const cur = try_get();
  if (cur !== null) {
    on_ready(cur);
    return () => {};
  }

  let done = false;

  const unsub = subscribe_wake(() => {
    if (done) return;

    const next = try_get();
    if (next === null) return;

    done = true;
    unsub();
    on_ready(next);
  });

  return () => {
    if (done) return;
    done = true;
    unsub();
  };
}

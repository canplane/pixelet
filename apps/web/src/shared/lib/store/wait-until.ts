// @pxl/web/shared/lib/store/wait-until.ts

export type Unsubscribe = () => void;
export type SubscribeWake = (wake: () => void) => Unsubscribe;

export function waitUntil<T>(tryGet: () => T | null, subscribeWake: SubscribeWake, onReady: (value: T) => void): Unsubscribe {
  const cur = tryGet();
  if (cur !== null) {
    onReady(cur);
    return () => {};
  }

  let done = false;

  const unsub = subscribeWake(() => {
    if (done) return;

    const next = tryGet();
    if (next === null) return;

    done = true;
    unsub();
    onReady(next);
  });

  return () => {
    if (done) return;
    done = true;
    unsub();
  };
}

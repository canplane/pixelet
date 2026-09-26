// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view backed by a Metal
 * layer (modules/pxl-surface), which the kernel draws into directly through
 * wgpu (crates/pxl/web/render/src/gpu/api/device.rs). A view is a kernel
 * surface as a page's canvas is (kernel/render/surface.ts): opened while it
 * is mounted, bound to the view's layer by the view itself while it has one —
 * it says when, since its layer comes after it is mounted and laid out — and
 * sized in device pixels as it is laid out.
 *
 * The world's view is also the scheduler's frame loop and the pointer's
 * source, as the web's driver is (kernel/control/input.ts): its size is the
 * frame's viewport, its touches are pointers the kernel reads into drags and
 * pinches, the turns run while it is up and the app is in front, and the
 * kernel hears when the app goes to the background — where it saves what was
 * placed — and, back in front, whether location was granted meanwhile.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { requireNativeView } from "expo";
import { type ComponentType, useEffect, useRef, useState } from "react";
import { AppState, type GestureResponderEvent, type LayoutChangeEvent, type NativeSyntheticEvent, PixelRatio, StyleSheet, type ViewProps } from "react-native";

import { Command, encode_sensor_visibility, encode_surface_create, encode_surface_delete, encode_surface_update, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_rect, kernel_touch } from "@/kernel/control/input.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame, ring_now, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";
import { permission_recheck } from "@platform/sensor.ts";

interface PxlSurfaceProps extends ViewProps {
  /** The kernel's surface to show; 0 for none. */
  surface: u32;
  /** The view bound its layer to `surface`. */
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

const PxlSurface: ComponentType<PxlSurfaceProps> = requireNativeView("PxlSurface");

/** A kernel surface of `role` for a PxlSurface view: opened while mounted and
 * again whenever `key` changes, `opened` handed it once it is the kernel's —
 * what that returns is run before it is let go — and sized in device pixels
 * once the view has bound its layer to it. `size` is the view's, in its own
 * pixels. */
function useSurface(role: u32, key: string, opened?: (surface: u32) => () => void) {
  const [handle, setHandle] = useState<u32>(0);
  const size = useRef<Viewport>({ width: 0, height: 0 });
  /** The surface the view says its layer is bound to; 0 for none. */
  const bound = useRef<u32>(0);
  const open = useRef(opened);
  open.current = opened;

  const send = () => {
    const surface = bound.current;
    if (surface === 0) return;
    const scale = PixelRatio.get();
    const { width, height } = size.current;
    CORE_BUILD.send(0, Command.SURFACE_UPDATE, encode_surface_update({ surface, width: round(width * scale), height: round(height * scale), device_pixel_ratio: scale }));
    request_frame();
  };

  useEffect(() => {
    const surface = CORE_BUILD.call(Command.SURFACE_CREATE, encode_surface_create({ role }), (result) => result >>> 0);
    setHandle(surface);
    const close = surface !== 0 && open.current ? open.current(surface) : () => {};
    return () => {
      close();
      setHandle(0);
      if (surface !== 0) CORE_BUILD.call(Command.SURFACE_DELETE, encode_surface_delete({ surface }), (result) => result);
    };
  }, [key]);

  const props: PxlSurfaceProps = {
    surface: handle,
    onLayout: (e: LayoutChangeEvent) => {
      size.current = { width: e.nativeEvent.layout.width, height: e.nativeEvent.layout.height };
      send();
    },
    onAttach: (e) => {
      bound.current = e.nativeEvent.surface;
      send();
    },
    onDetach: () => {
      bound.current = 0;
    },
  };
  return { size, props };
}

export function WorldSurface() {
  const { size, props } = useSurface(surface_role.WORLD, "world");

  useEffect(() => {
    set_driver(() => size.current);
    ring_now(Command.SENSOR_OPEN);
    strobe.start();
    const visibility = AppState.addEventListener("change", (state) => {
      const visible = state === "active";
      CORE_BUILD.send(0, Command.SENSOR_VISIBILITY, encode_sensor_visibility({ visible }));
      CORE_BUILD.ring(0);
      if (visible) {
        permission_recheck();
        strobe.start();
      } else strobe.stop();
    });
    return () => {
      visibility.remove();
      strobe.stop();
      ring_now(Command.SENSOR_CLOSE);
      set_driver(null);
    };
  }, []);

  const onLayout = (e: LayoutChangeEvent) => {
    props.onLayout?.(e);
    // A touch's place is the view's own (`locationX`), so the view is the
    // frame it is read against, wherever the view lies.
    kernel_rect(0, 0, e.nativeEvent.layout.width, e.nativeEvent.layout.height);
    request_frame();
  };

  /** Every touch the event changed, as a pointer in `phase`. */
  const touches = (phase: PointerPhase) => (e: GestureResponderEvent) => {
    for (const t of e.nativeEvent.changedTouches) kernel_touch(phase, Number(t.identifier), t.locationX, t.locationY);
  };

  return (
    <PxlSurface
      {...props}
      style={StyleSheet.absoluteFill}
      onLayout={onLayout}
      onTouchStart={touches(PointerPhase.Down)}
      onTouchMove={touches(PointerPhase.Move)}
      onTouchEnd={touches(PointerPhase.Up)}
      onTouchCancel={touches(PointerPhase.Up)}
    />
  );
}

/** A display case: a surface the kernel draws one thing into. `onOpen` is
 * handed the surface once it is the kernel's, and what it returns is run
 * before the surface is let go; it opens again whenever `openKey` changes. */
export function CaseSurface({ onOpen, openKey }: { onOpen: (surface: u32) => () => void; openKey: string }) {
  const { props } = useSurface(surface_role.CASE, openKey, onOpen);
  return <PxlSurface {...props} style={StyleSheet.absoluteFill} />;
}

// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view backed by a Metal
 * layer (modules/platform/ios/SurfaceView.swift), which the kernel draws into
 * directly through wgpu (crates/pxl/web/render/src/gpu/api/device.rs). A
 * view is a kernel surface as a page's canvas is (platform/web/surface.tsx):
 * opened while it is mounted, bound to the view's layer by the view itself
 * while it has one — it says when, since its layer comes after it is mounted
 * and laid out — and sized in device pixels as it is laid out.
 *
 * The world's view is also the app's driver (kernel/control/input.ts), as a
 * page's canvas is: its size is the frame's viewport, its touches are
 * pointers the kernel reads into drags and pinches, and the app in front or
 * in the background is the kernel's visibility.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { requireNativeView } from "expo";
import { type ComponentType, useEffect, useRef, useState } from "react";
import { AppState, type GestureResponderEvent, type LayoutChangeEvent, type NativeSyntheticEvent, PixelRatio, StyleSheet, type ViewProps } from "react-native";

import { PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_open, kernel_pointer, kernel_rect, kernel_visible } from "@/kernel/control/input.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import { surface_create, surface_delete, surface_size } from "@/kernel/render/surface.ts";
import { request_frame, type Viewport } from "@/kernel/sched.ts";

interface SurfaceProps extends ViewProps {
  /** The kernel's surface to show; 0 for none. */
  surface: u32;
  /** The view bound its layer to `surface`. */
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

const SurfaceView: ComponentType<SurfaceProps> = requireNativeView("PxlPlatform");

/** A kernel surface of `role` for a SurfaceView: opened while mounted and
 * again whenever `key` changes, `opened` handed it once it is the kernel's —
 * what that returns is run before it is let go — and sized in device pixels
 * once the view has bound its layer to it. `size` is the view's, in its own
 * pixels. */
function useSurface(role: u32, key: string, opened?: (surface: u32) => () => void) {
  const [handle, setHandle] = useState<u32>(0);
  const size = useRef<Viewport>({ width: 0, height: 0, dpr: PixelRatio.get() });
  /** The surface the view says its layer is bound to; 0 for none. */
  const bound = useRef<u32>(0);
  const open = useRef(opened);
  open.current = opened;

  const send = () => {
    const surface = bound.current;
    if (surface === 0) return;
    const { width, height, dpr } = size.current;
    surface_size(surface, round(width * dpr), round(height * dpr), dpr);
  };

  useEffect(() => {
    const surface = surface_create(role);
    setHandle(surface);
    const close = surface !== 0 && open.current ? open.current(surface) : () => {};
    return () => {
      close();
      setHandle(0);
      if (surface !== 0) surface_delete(surface);
    };
  }, [key]);

  const props: SurfaceProps = {
    surface: handle,
    onLayout: (e: LayoutChangeEvent) => {
      size.current = { width: e.nativeEvent.layout.width, height: e.nativeEvent.layout.height, dpr: PixelRatio.get() };
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
    const close = kernel_open(() => size.current);
    const visibility = AppState.addEventListener("change", (state) => kernel_visible(state === "active"));
    return () => {
      visibility.remove();
      close();
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
    for (const t of e.nativeEvent.changedTouches) kernel_pointer(phase, PointerKind.Touch, Number(t.identifier), 0, t.locationX, t.locationY);
  };

  return (
    <SurfaceView
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
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

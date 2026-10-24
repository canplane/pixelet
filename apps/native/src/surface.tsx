// @pxl/native/surface.tsx

/**
 * The kernel's surface on a device: a native view (SurfaceView.swift,
 * KernelSurfaceView.kt) drawn by wgpu directly. The view says when its layer is bound, since the
 * layer arrives after mount. The world's view is also the app's input driver.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { requireNativeView } from "expo";
import { type ComponentType, useEffect, useRef, useState } from "react";
import { AppState, type GestureResponderEvent, type LayoutChangeEvent, type NativeSyntheticEvent, PixelRatio, StyleSheet, type ViewProps } from "react-native";

import { PointerPhase, PointerType } from "@pxl/client/vmm/abi/cmd.spec.ts";
import { kernel_open, kernel_rect, kernel_visible } from "@pxl/client/vmm/control/input.ts";
import { worldPointer } from "@pxl/client/shell/store/pointer.ts";
import { surface_role } from "@pxl/client/vmm/render/gpu/api.spec.ts";
import { surface_close, surface_open, surface_size } from "@pxl/client/vmm/render/surface.ts";
import { request_frame, type Viewport } from "@pxl/client/vmm/sched.ts";

interface SurfaceProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

const SurfaceView: ComponentType<SurfaceProps> = requireNativeView("PxlPlatform");

function useSurface(role: u32, key: string, opened?: (surface: u32) => () => void) {
  const [handle, setHandle] = useState<u32>(0);
  const size = useRef<Viewport>({ width: 0, height: 0, dpr: PixelRatio.get() });
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
    const surface = surface_open(role);
    setHandle(surface);
    const close = surface !== 0 && open.current ? open.current(surface) : () => {};
    return () => {
      close();
      setHandle(0);
      if (surface !== 0) surface_close(surface);
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
    kernel_rect(0, 0, e.nativeEvent.layout.width, e.nativeEvent.layout.height);
    request_frame();
  };

  const touches = (phase: PointerPhase) => (e: GestureResponderEvent) => {
    for (const t of e.nativeEvent.changedTouches) worldPointer(phase, PointerType.Touch, Number(t.identifier), 0, t.locationX, t.locationY);
  };

  return (
    <SurfaceView
      {...props}
      style={StyleSheet.absoluteFill}
      onLayout={onLayout}
      onTouchStart={touches(PointerPhase.Down)}
      onTouchMove={touches(PointerPhase.Move)}
      onTouchEnd={touches(PointerPhase.Up)}
      onTouchCancel={touches(PointerPhase.Cancel)}
    />
  );
}

export function CaseSurface({ onOpen, openKey }: { onOpen: (surface: u32) => () => void; openKey: string }) {
  const { props } = useSurface(surface_role.CASE, openKey, onOpen);
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

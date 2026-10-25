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

import type { CaseSurfaceProps, PointerPhase, SurfaceDriver, Viewport, WorldSurfaceProps } from "@pxl/client/bracket.tsx";

interface SurfaceProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

const SurfaceView: ComponentType<SurfaceProps> = requireNativeView("PxlPlatform");

function useSurface(driver: SurfaceDriver, key: string, opened?: (surface: u32) => () => void) {
  const [handle, setHandle] = useState<u32>(0);
  const size = useRef<Viewport>({ width: 0, height: 0, dpr: PixelRatio.get() });
  const bound = useRef<u32>(0);
  const open = useRef(opened);
  open.current = opened;

  const send = () => {
    const surface = bound.current;
    if (surface === 0) return;
    const { width, height, dpr } = size.current;
    driver.size(surface, round(width * dpr), round(height * dpr), dpr);
  };

  useEffect(() => {
    const surface = driver.open();
    setHandle(surface);
    const close = surface !== 0 && open.current ? open.current(surface) : () => {};
    return () => {
      close();
      setHandle(0);
      if (surface !== 0) driver.close(surface);
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

export function World({ surface: driver, input }: WorldSurfaceProps) {
  const { size, props } = useSurface(driver, "world");

  useEffect(() => {
    const close = input.open(() => size.current);
    const visibility = AppState.addEventListener("change", (state) => input.visible(state === "active"));
    return () => {
      visibility.remove();
      close();
    };
  }, []);

  const onLayout = (e: LayoutChangeEvent) => {
    props.onLayout?.(e);
    input.rect(0, 0, e.nativeEvent.layout.width, e.nativeEvent.layout.height);
    input.requestFrame();
  };

  const touches = (phase: PointerPhase) => (e: GestureResponderEvent) => {
    for (const t of e.nativeEvent.changedTouches) input.pointer(phase, "touch", Number(t.identifier), 0, t.locationX, t.locationY);
  };

  return (
    <SurfaceView
      {...props}
      style={StyleSheet.absoluteFill}
      onLayout={onLayout}
      onTouchStart={touches("down")}
      onTouchMove={touches("move")}
      onTouchEnd={touches("up")}
      onTouchCancel={touches("cancel")}
    />
  );
}

export function Case({ surface: driver, onOpen, openKey }: CaseSurfaceProps) {
  const { props } = useSurface(driver, openKey, onOpen);
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

// @pxl/client/service/surface.native.tsx

/**
 * The kernel's surface on a device: xpute's native view (@xpute/native
 * `SurfaceView`) drawn by wgpu directly. The view says when its layer is bound, since the
 * layer arrives after mount. The world's view is also the app's input driver.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { SurfaceView, type SurfaceViewProps } from "@xpute/native";
import { useEffect, useRef, useState } from "react";
import {
  type GestureResponderEvent,
  type LayoutChangeEvent,
  PixelRatio,
  StyleSheet,
} from "react-native";

import type {
  CharacterSurfaceProps,
  PointerPhase,
  SurfaceDriver,
  Viewport,
  WorldSurfaceProps,
} from "./types.ts";

function useSurface(
  driver: SurfaceDriver,
  key: string,
  opened?: (surface: u32) => () => void,
) {
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
    let surface: u32 = 0;
    let close = () => {};
    let gone = false;
    void driver.open().then((opened) => {
      if (opened === 0) return;
      // Closed before the kernel answered: the handle is let go at once.
      if (gone) return driver.close(opened);
      surface = opened;
      setHandle(surface);
      if (open.current) close = open.current(surface);
    });
    return () => {
      gone = true;
      close();
      setHandle(0);
      if (surface !== 0) driver.close(surface);
    };
  }, [key]);

  const props: SurfaceViewProps = {
    surface: handle,
    onLayout: (e: LayoutChangeEvent) => {
      size.current = {
        width: e.nativeEvent.layout.width,
        height: e.nativeEvent.layout.height,
        dpr: PixelRatio.get(),
      };
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
  const { props } = useSurface(driver, "world");

  useEffect(() => input.open(), []);

  const onLayout = (e: LayoutChangeEvent) => {
    props.onLayout?.(e);
    input.requestFrame();
  };

  const touches =
    (...phases: PointerPhase[]) =>
    (e: GestureResponderEvent) => {
      for (const t of e.nativeEvent.changedTouches) {
        for (const phase of phases) {
          input.pointer(
            phase,
            "touch",
            Number(t.identifier),
            0,
            t.locationX,
            t.locationY,
          );
        }
      }
    };

  return (
    <SurfaceView
      {...props}
      style={StyleSheet.absoluteFill}
      onLayout={onLayout}
      onTouchStart={touches("down")}
      onTouchMove={touches("move")}
      // A lifted finger leaves the surface, as the web's pointerleave says
      // after its pointerup: nothing stays hovered under it.
      onTouchEnd={touches("up", "out")}
      onTouchCancel={touches("cancel", "out")}
    />
  );
}

export function CharacterSurface({
  surface: driver,
  onOpen,
  openKey,
}: CharacterSurfaceProps) {
  const { props } = useSurface(driver, openKey, onOpen);
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

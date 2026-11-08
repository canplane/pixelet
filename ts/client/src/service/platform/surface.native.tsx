// @pxl/client/service/platform/surface.native.tsx

/** The core's surfaces on a device are xpute's native view (@xpute/native
 * `SurfaceView`), which wgpu draws into directly. The view's layer arrives
 * after the view mounts, so the view reports when its layer is bound. */

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
  const attached = useRef<u32>(0);
  const open = useRef(opened);
  open.current = opened;

  const send = () => {
    const surface = attached.current;
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
      // The surface closed before the core answered, so the handle is let go
      // at once.
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
      attached.current = e.nativeEvent.surface;
      send();
    },
    onDetach: () => {
      attached.current = 0;
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
      // A lifted finger also leaves the surface, as a page's pointerleave
      // follows its pointerup, so that nothing stays hovered under it.
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

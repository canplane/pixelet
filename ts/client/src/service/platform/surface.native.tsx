// @pxl/client/service/platform/surface.native.tsx

/** The core's surfaces natively are the GPU's native view (@pxl/gpu/expo
 * `SurfaceView`), which wgpu draws into directly. The view's layer arrives
 * after the view mounts, so the view reports when its layer is bound. A
 * native world hears no wheel and no keys. */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { SurfaceView, type SurfaceViewProps } from "@pxl/gpu/expo";
import { useEffect, useRef, useState } from "react";
import {
  type LayoutChangeEvent,
  Dimensions,
  PixelRatio,
  StyleSheet,
} from "react-native";

import type {
  CharacterSurfaceProps,
  SurfaceDriver,
  Viewport,
  WorldSurfaceProps,
} from "./types.ts";

/** No press is shifted, with no keyboard to shift it. */
export function pressShifted(): boolean {
  return false;
}

/** The surface fills the window. */
export function surfaceBox(): { width: f64; height: f64 } {
  const { width, height } = Dimensions.get("window");
  return { width, height };
}

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
    let closed = false;
    void driver.open().then((opened) => {
      if (opened === 0) return;
      // The surface closed before the core answered, so the handle is let go
      // at once.
      if (closed) return driver.close(opened);
      surface = opened;
      setHandle(surface);
      if (open.current) close = open.current(surface);
    });
    return () => {
      closed = true;
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
  return props;
}

export function World({ surface: driver }: WorldSurfaceProps) {
  const props = useSurface(driver, "world");
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

export function CharacterSurface({
  surface: driver,
  onOpen,
  openKey,
}: CharacterSurfaceProps) {
  const props = useSurface(driver, openKey, onOpen);
  return <SurfaceView {...props} style={StyleSheet.absoluteFill} />;
}

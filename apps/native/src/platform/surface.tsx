// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view backed by a Metal
 * layer (modules/pxl-surface), which the kernel draws into directly through
 * wgpu (crates/pxl/web/native). Mounted, it is the scheduler's frame loop, as
 * the web's driver is (kernel/control/input.ts): its size is the frame's
 * viewport, the turns run while it is up and the app is in front, and the
 * kernel hears when the app goes to the background — where it saves what was
 * placed. A display case has no surface of its own yet.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { requireNativeView } from "expo";
import { type ComponentType, useEffect, useRef } from "react";
import { AppState, type LayoutChangeEvent, StyleSheet, View, type ViewProps } from "react-native";

import { Command, encode_sensor_visibility } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame, ring_now, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";

const PxlSurface: ComponentType<ViewProps> = requireNativeView("PxlSurface");

export function WorldSurface() {
  const size = useRef<Viewport>({ width: 0, height: 0 });

  useEffect(() => {
    set_driver(() => size.current);
    ring_now(Command.SENSOR_OPEN);
    strobe.start();

    const visibility = AppState.addEventListener("change", (state) => {
      const visible = state === "active";
      CORE_BUILD.send(0, Command.SENSOR_VISIBILITY, encode_sensor_visibility({ visible }));
      CORE_BUILD.ring(0);
      if (visible) strobe.start();
      else strobe.stop();
    });

    return () => {
      visibility.remove();
      strobe.stop();
      ring_now(Command.SENSOR_CLOSE);
      set_driver(null);
    };
  }, []);

  const onLayout = (e: LayoutChangeEvent) => {
    size.current = { width: e.nativeEvent.layout.width, height: e.nativeEvent.layout.height };
    request_frame();
  };

  return <PxlSurface style={StyleSheet.absoluteFill} onLayout={onLayout} />;
}

export function CaseSurface(_props: { onOpen: (surface: u32) => () => void; openKey: string }) {
  return <View style={StyleSheet.absoluteFill} />;
}

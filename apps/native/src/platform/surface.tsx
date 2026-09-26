// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view backed by a Metal
 * layer (modules/pxl-surface), which the kernel draws into directly through
 * wgpu (crates/pxl/web/render/src/gpu/api/device.rs). Mounted, it is the
 * kernel's world surface as a page's canvas is (kernel/render/surface.ts):
 * opened on mount, bound to the view's layer while the view has one — the
 * view says when, since its layer comes after it is mounted and laid out —
 * and sized in device pixels as the view is laid out. It is also the scheduler's frame loop, as the web's driver is
 * (kernel/control/input.ts): its size is the frame's viewport, the turns run
 * while it is up and the app is in front, and the kernel hears when the app
 * goes to the background — where it saves what was placed. A display case has
 * no surface of its own yet.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { requireNativeView } from "expo";
import { type ComponentType, useEffect, useRef } from "react";
import { AppState, type LayoutChangeEvent, PixelRatio, StyleSheet, View, type ViewProps } from "react-native";

import { Command, encode_sensor_visibility, encode_surface_create, encode_surface_delete, encode_surface_update } from "@/kernel/abi/cmd.spec.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame, ring_now, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";
import { PXL_KERNEL } from "./core.ts";

interface PxlSurfaceProps extends ViewProps {
  onAttach: () => void;
  onDetach: () => void;
}

const PxlSurface: ComponentType<PxlSurfaceProps> = requireNativeView("PxlSurface");

export function WorldSurface() {
  const size = useRef<Viewport>({ width: 0, height: 0 });
  /** The kernel's surface, and whether the view's layer is bound to it. */
  const surface = useRef({ handle: 0, bound: false });

  /** Tells the kernel the surface's size in device pixels, once it is bound. */
  const update = () => {
    const s = surface.current;
    if (!s.bound) return;
    const scale = PixelRatio.get();
    const { width, height } = size.current;
    CORE_BUILD.send(0, Command.SURFACE_UPDATE, encode_surface_update({ surface: s.handle, width: round(width * scale), height: round(height * scale), device_pixel_ratio: scale }));
    request_frame();
  };

  useEffect(() => {
    set_driver(() => size.current);
    ring_now(Command.SENSOR_OPEN);
    strobe.start();
    surface.current.handle = CORE_BUILD.call(Command.SURFACE_CREATE, encode_surface_create({ role: surface_role.WORLD }), (result) => result >>> 0);

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
      const { handle, bound } = surface.current;
      if (bound) PXL_KERNEL.surfaceUnbind(handle);
      if (handle !== 0) CORE_BUILD.call(Command.SURFACE_DELETE, encode_surface_delete({ surface: handle }), (result) => result);
      surface.current = { handle: 0, bound: false };
      set_driver(null);
    };
  }, []);

  const onLayout = (e: LayoutChangeEvent) => {
    size.current = { width: e.nativeEvent.layout.width, height: e.nativeEvent.layout.height };
    update();
    request_frame();
  };

  const onAttach = () => {
    const s = surface.current;
    s.bound = s.handle !== 0 && PXL_KERNEL.surfaceBind(s.handle);
    update();
  };

  const onDetach = () => {
    const s = surface.current;
    if (s.bound) PXL_KERNEL.surfaceUnbind(s.handle);
    s.bound = false;
  };

  return <PxlSurface style={StyleSheet.absoluteFill} onLayout={onLayout} onAttach={onAttach} onDetach={onDetach} />;
}

export function CaseSurface(_props: { onOpen: (surface: u32) => () => void; openKey: string }) {
  return <View style={StyleSheet.absoluteFill} />;
}

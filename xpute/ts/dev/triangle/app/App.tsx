// xpute-triangle-app/App.tsx

/**
 * The same core as the page's (rust/dev/triangle), linked into the app. The app
 * starts it, opens the GPU, opens a surface and shows it in xpute's view, and
 * tells the core the view's size. The core draws every frame from then on.
 */

import { Command, type GpuProtocol } from "@xpute/gpu/abi.spec.ts";
import { gpu_protocol } from "@xpute/gpu/gpu.ts";
import { NativeGpu, SurfaceView } from "@xpute/gpu/native.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { FramePacing, type Manifest, QUIET, start } from "@xpute/runtime";
import { useEffect, useRef, useState } from "react";
import { type LayoutChangeEvent, PixelRatio, StyleSheet } from "react-native";
import { MACHINE } from "../machine.spec.ts";

/** The major the program gives the GPU's commands, which the core names too. */
const GPU_MAJOR = 0x10;

const manifest: Manifest = {
  machine: MACHINE,
  core: () => ({
    policy: new FramePacing({
      pace_hz: 60,
      margin_share: 0.15,
      batch_frames: 2,
      settle_ms: 150,
    }),
    submit: () => {},
  }),
};

async function open(): Promise<{ gpu: GpuProtocol; surface: number }> {
  const link = await start(manifest, QUIET);
  const gpu = gpu_protocol(link, GPU_MAJOR);
  new NativeGpu().grant({ ready: () => gpu.send(Command.GPU_READY) });
  link.resume();
  return {
    gpu,
    surface: Math.max(0, await gpu.request(Command.GPU_SURFACE_OPEN)),
  };
}

export function App() {
  const [surface, setSurface] = useState(0);
  const gpu = useRef<GpuProtocol | null>(null);
  const attached = useRef(0);
  const box = useRef({ width: 0, height: 0 });

  const size = () => {
    if (attached.current === 0) return;
    const dpr = PixelRatio.get();
    gpu.current?.send(
      Command.GPU_SURFACE_RESIZE,
      attached.current,
      round(box.current.width * dpr),
      round(box.current.height * dpr),
      dpr,
    );
  };

  useEffect(() => {
    void open().then((opened) => {
      gpu.current = opened.gpu;
      setSurface(opened.surface);
    });
  }, []);

  return (
    <SurfaceView
      style={StyleSheet.absoluteFill}
      surface={surface}
      onLayout={(e: LayoutChangeEvent) => {
        box.current = e.nativeEvent.layout;
        size();
      }}
      onAttach={(e) => {
        attached.current = e.nativeEvent.surface;
        size();
      }}
      onDetach={() => {
        attached.current = 0;
      }}
    />
  );
}

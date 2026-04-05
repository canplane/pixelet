// @pxl/web/modules/world/frame/context.tsx

import { createContext, type ReactNode, type RefObject, useContext, useRef } from "react";
import * as THREE from "three";

export type FrameRefs = {
  world_frame_ref: RefObject<THREE.Group | null>;
  local_world_frame_ref: RefObject<THREE.Group | null>;
};

export const FrameContext = createContext<FrameRefs | null>(null);

export function useFrameRefs(): FrameRefs {
  const ctx = useContext(FrameContext);
  if (!ctx) throw new Error("useFrameRefs must be used under <FrameContext.Provider>");
  return ctx;
}

type FrameProviderProps = {
  children: ReactNode;
};
export function FrameProvider(props: FrameProviderProps) {
  const { children } = props;

  const world_frame_ref = useRef<THREE.Group>(null);
  const local_world_frame_ref = useRef<THREE.Group>(null);

  return <FrameContext.Provider value={{ world_frame_ref, local_world_frame_ref }}>{children}</FrameContext.Provider>;
}

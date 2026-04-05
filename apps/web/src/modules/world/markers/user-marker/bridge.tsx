// @pxl/web/modules/world/markers/bridge.tsx

import { useLayoutEffect, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

import { L3_IDX } from "@pxl/spec/space/indexer.ts";
import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";

import { useTileStore } from "../../terrain/tile-store.ts";
import { get_theme_key, WorldThemeKey } from "../../render/themes.ts";
import { useAnchorRegistry } from "../../spatial/anchor/context.tsx";
import { UserMarkerObject } from "./object.ts";

type UserMarkerProps = {
  device_coord: PVF3;
};
export function UserMarker(props: UserMarkerProps) {
  const { device_coord } = props;

  const { camera, clock } = useThree();

  const registry = useAnchorRegistry();
  const obj_ref = useRef<UserMarkerObject | null>(null);

  const theme_key = get_theme_key();
  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;

  const tile = useTileStore((state) => state.get_tile(L3_IDX(device_coord.i)));

  useLayoutEffect(() => {
    const obj = new UserMarkerObject(registry);
    obj_ref.current = obj;

    return () => {
      obj.dispose();
      obj_ref.current = null;
    };
  }, [registry]);

  useLayoutEffect(() => {
    obj_ref.current?.set_theme(is_night);
  }, [is_night]);

  useLayoutEffect(() => {
    obj_ref.current?.set_coord(device_coord);
  }, [device_coord, tile]);

  useFrame(() => {
    const obj = obj_ref.current;
    if (!obj) return;
    if (!(camera instanceof THREE.PerspectiveCamera)) return;

    obj.tick(camera, clock.getElapsedTime());
  });

  return null;
}

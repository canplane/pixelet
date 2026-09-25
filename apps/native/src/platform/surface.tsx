// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view backed by a Metal
 * layer (modules/pxl-surface), which the kernel draws into directly through
 * wgpu (crates/pxl/web/native). A display case has no surface of its own yet.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { requireNativeView } from "expo";
import type { ComponentType } from "react";
import { StyleSheet, View, type ViewProps } from "react-native";

const PxlSurface: ComponentType<ViewProps> = requireNativeView("PxlSurface");

export function WorldSurface() {
  return <PxlSurface style={StyleSheet.absoluteFill} />;
}

export function CaseSurface(_props: { onOpen: (surface: u32) => () => void; openKey: string }) {
  return <View style={StyleSheet.absoluteFill} />;
}

// @pxl/native/platform/surface.tsx

/**
 * What the kernel draws into, on a device: a native view it will draw into
 * directly once it is built for one. Until then, a plain ground under the
 * chrome.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { StyleSheet, View } from "react-native";

export function WorldSurface() {
  return <View style={[StyleSheet.absoluteFill, styles.world]} />;
}

export function CaseSurface(_props: { onOpen: (surface: u32) => () => void; openKey: string }) {
  return <View style={StyleSheet.absoluteFill} />;
}

const styles = StyleSheet.create({
  world: { backgroundColor: "#8fb07f" },
});

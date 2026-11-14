// @pxl/client/service/platform/share.native.ts

import { File, Paths } from "expo-file-system";
import { Share } from "react-native";

import { PLATFORM } from "./pxl-platform.native.ts";

export async function save(
  name: string,
  text: string,
  _mime: string,
): Promise<void> {
  const file = new File(Paths.cache, name);
  file.create({ overwrite: true });
  file.write(text);
  await Share.share({ url: file.uri });
}

export function copy(text: string): Promise<void> {
  PLATFORM.clipboard_set(text);
  return Promise.resolve();
}

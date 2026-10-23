// @pxl/native/save.ts

import { File, Paths } from "expo-file-system";
import { Share } from "react-native";

import { PLATFORM } from "./module.ts";

/** Written to the cache and offered through the share sheet. */
export async function saveFile(name: string, text: string, _mime: string): Promise<void> {
  const file = new File(Paths.cache, name);
  file.create({ overwrite: true });
  file.write(text);
  await Share.share({ url: file.uri });
}

export function copyText(text: string): Promise<void> {
  PLATFORM.clipboardSet(text);
  return Promise.resolve();
}

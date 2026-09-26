// @pxl/native/platform/save.ts

import { File, Paths } from "expo-file-system";
import { Share } from "react-native";

import { PLATFORM } from "./module.ts";

/** Hands the user `text` as a file named `name`: on a device, written to the
 * app's cache and offered through the system's share sheet, from which it
 * goes to Files, AirDrop or another app. The type is the file's extension's
 * to the system. */
export async function save_file(name: string, text: string, _mime: string): Promise<void> {
  const file = new File(Paths.cache, name);
  file.create({ overwrite: true });
  file.write(text);
  await Share.share({ url: file.uri });
}

/** Puts `text` on the clipboard: the system's pasteboard, which a device's
 * JavaScript has no way to reach but through the app. */
export function copy_text(text: string): Promise<void> {
  PLATFORM.clipboardSet(text);
  return Promise.resolve();
}

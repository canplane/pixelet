// @pxl/native/platform/text-field.ts

import { Platform } from "react-native";

/** Whether the system text field has its own clear button: iOS does (while
 * editing), Android does not. */
export const SYSTEM_CLEAR: boolean = Platform.OS === "ios";

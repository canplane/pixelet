// @xpute/web-runtime

export { tell_life } from "./life.ts";
export {
  type Instantiate,
  type InstantiateError,
  type Log,
  type Stopped,
  type Trap,
} from "./message.ts";
export { open_worker } from "./page.ts";
export { forward_console, instantiate } from "./worker.ts";

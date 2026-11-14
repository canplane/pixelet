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
export { type Taker } from "./remote.ts";
export {
  type Boot,
  forward_console,
  instantiate,
  type Server,
  start,
} from "./worker.ts";

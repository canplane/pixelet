// @xpute/runtime

export { serve_clock } from "./clock.ts";
export { tell_life } from "./life.ts";
export {
  type Answer,
  type Answered,
  type Core,
  type Handle,
  Link,
  type LinkObject,
  type Method,
  OPEN,
  open,
  type Opener,
  type Packet,
  RELEASE,
  release,
  refusal_of,
  type Ref,
  type Reply,
  ROOT,
  type Sender,
  type Tag,
  uri,
} from "./link.ts";
export { Remote, serve, type Taker } from "./remote.ts";
export { Table } from "./resource.ts";
export { type Stop, serve_stop } from "./stop.ts";
export { serve_timer } from "./timer.ts";
export { wait } from "./wait.ts";

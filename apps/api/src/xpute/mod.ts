// @pxl/api/xpute/mod.ts

import { CommandPort } from "@xpute/spec/control/post.ts";
import { NetGateway } from "@xpute/core/net/gateway.ts";
import { TransportKind, type Transports } from "@xpute/spec/net/xport/mod.ts";
import type { XputeContext } from "@xpute/spec/ipc/link.ts";
import type { GuestBootstrapper } from "@xpute/core/ipc/worker/contract.ts";

import { CentralTransport } from "@xpute/core/net/xport/central.ts";
import { WSPlug } from "./plug/ws.ts";
import { HTTPPlug } from "./plug/http.ts";

import { MeshTransport } from "@xpute/core/net/xport/mesh.ts";
import { RTCSignalRelay } from "./plug/rtc.signal_relay.ts";
import { RTCSignalGateway } from "./plug/rtc.signal_gateway.ts";

import { set_xpute } from "@/xpute/context.ts";
import * as env from "@/lib/env/mod.ts";

import { main } from "@/main.ts";

/**
 * 런타임 환경 변수 및 설정 검증
 * 검증에 실패하면 즉시 process를 중단(Panic)합니다.
 */
function safety_check() {
  const network_type = env.must_one_of("NETWORK_TYPE", ["testnet", "mainnet"] as const, "testnet");
  const safe_mode = env.must_bool("SAFE_MODE", { fallback: false });
  if (safe_mode && network_type !== "testnet") {
    throw new Error("safety check: safe mode can only be enabled on testnet.");
  }

  const hello_cookie_secret = env.must_match(
    "HELLO_COOKIE_SECRET_B64URL",
    /^[A-Za-z0-9_-]+$/,
    "Cookie secret must be a valid Base64URL string.",
  );

  // 3. Security Review Log
  console.info(`safety check: build assumes threat model: ${env.get("THREAT_MODEL") || "DEFAULT"}`);

  // 4. Clock Sanity Warning
  // 시스템 시간이 최소한 프로젝트 릴리즈 시점보다는 이후인지 체크
  if (Date.now() < new Date("2026-03-25").getTime()) {
    console.warn("safety check: ⚠️ system clock seems desynchronized. This may cause network issues.");
  }

  // 검증된 값을 반환하여 재사용
  return { hello_cookie_secret };
}

const bootstrap: GuestBootstrapper = async (tx, net_uplink) => {
  console.debug("xpute: guest starting...");

  // 여기서 검증과 동시에 값을 추출
  const env_safe = safety_check();

  // TODO(ops):
  // - env-driven safe mode (testnet-only, verbose logging, outbound kill switch)
  // - startup self-checks: cookie secret presence/format, seed dial validation, clock sanity warning
  // - security review doc pointer/version tag log on boot (which threat model this build assumes)

  // ---- post ----

  const post = new CommandPort(tx);

  const xpute: XputeContext = { post };

  // ---- transports ----

  const ws = new WSPlug();
  const http = new HTTPPlug();
  const rtc_signal_relay = new RTCSignalRelay();
  const rtc_signal_gateway = new RTCSignalGateway(rtc_signal_relay);

  set_xpute(xpute);
  await main((ctx) => {
    const { router, io } = ctx;
    http.attach(router);
    ws.attach(io);
    rtc_signal_gateway.attach(io);
  });

  // ---- network gateway ----

  const xports = {
    [TransportKind.CENTRAL]: new CentralTransport(ws, http, 10_000),
    [TransportKind.MESH]: new MeshTransport(rtc_signal_relay),
  } as const satisfies Transports;

  const net_gateway = new NetGateway(net_uplink, xports, {
    hello_cookie_secret: env_safe.hello_cookie_secret,
  });

  // handover
  return { post, net_gateway };
};

export default bootstrap;

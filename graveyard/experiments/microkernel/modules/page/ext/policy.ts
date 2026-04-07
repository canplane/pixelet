// // @pxl/engine/src/modules/page/ext/policy.ts

// import { TransportPolicy } from "@xpute/spec/rpc/transport.ts";

// /**
//  * [Default Policy]
//  * 기본 서버-클라이언트 통신용 운송장입니다.
//  */
// export const PAGE_DEFAULT_POLICY: Record<string, TransportPolicy> = {
//   // 'base' 레이어에 대한 조달 계획
//   base: {
//     strategy: "BLOCK", // HTTP/3 화물 운송
//     endpoint: "page/terrain/all", // 논리적 주소
//     priority: "CRITICAL", // 최우선 순위
//   },
// };

// /**
//  * [P2P Policy] (Optional)
//  * 서버가 터졌거나 로컬 피어가 있을 때 사용하는 운송장입니다.
//  * 설계도(Manifest)는 똑같지만, 가져오는 방식만 바뀝니다.
//  */
// export const PAGE_P2P_POLICY: Record<string, TransportPolicy> = {
//   base: {
//     strategy: "P2P", // 옆 노드에게 직접 요청
//     endpoint: "peer/page/base",
//     priority: "HIGH",
//   },
// };

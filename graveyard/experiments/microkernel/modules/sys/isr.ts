// // @pxl/engine/src/modules/sys/isr.ts

// import { ISR } from "@xpute/spec/control/isr.ts";
// import { SysAuth } from "./ext/auth.ts";

// /**
//  * [System ISR Implementation]
//  * 시스템 제어(Boot, Sync, Auth)를 담당하는 핸들러 구현체입니다.
//  */

// // =============================================================================
// // [Core Implementation]
// // =============================================================================

// /**
//  * [boot] (0x00)
//  * 시스템 부팅 핸들러입니다.
//  * Payload: [ Version(U32) ]
//  * Return:  [ ServerTime(F64) ]
//  */
// export const boot: ISR = async (signal) => {
//   const view = new DataView(signal.buffer, signal.byteOffset, signal.byteLength);

//   // 1. Packet Integrity Check (from meta.ts)
//   // ABI 규격에 따라 최소 4바이트(Version U32)가 필요합니다.
//   if (signal.byteLength < 4) throw new Error("Kernel Panic: Boot Packet Corrupted");

//   // 2. Version Check (Static Access)
//   const clientVersion = view.getUint32(0, false); // Big Endian
//   if (!SysAuth.checkVersion(clientVersion)) {
//     // 보안상 이유로 연결 종료 유도
//     throw new Error(`Kernel Panic: Version Mismatch (Client: ${clientVersion})`);
//   }

//   // 3. ACK with Server Time
//   // 클라이언트는 이 시간을 보고 초기 시계를 맞춥니다.
//   const response = new Uint8Array(8);
//   new DataView(response.buffer).setFloat64(0, Date.now(), false);
//   return response;
// };

// /**
//  * [sync] (0x01)
//  * 시간 동기화 핸들러입니다.
//  * Return: [ ServerTime(F64) ]
//  */
// export const sync: ISR = async () => {
//   // RTT 계산을 위해 서버의 현재 시간을 즉시 반환합니다.
//   const response = new Uint8Array(8);
//   new DataView(response.buffer).setFloat64(0, Date.now(), false);
//   return response;
// };

// /**
//  * [auth] (0x02)
//  * 유저 인증 핸들러입니다.
//  * Payload: [ Token(Variable) ]
//  * Return:  [ UserID(Bytes) ]
//  */
// export const auth: ISR = async (signal) => {
//   // [Static Access] 인증 모듈에 토큰 검증 위임
//   const userId = await SysAuth.authenticate(signal);

//   if (!userId) throw new Error("Kernel Panic: Auth Failed");

//   // 성공 시 유저 식별자 반환
//   return new TextEncoder().encode(userId);
// };

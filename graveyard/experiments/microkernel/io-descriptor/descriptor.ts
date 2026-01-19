// // @xpute/spec/io/descriptor.ts

// import type { FrameDef } from "./manifest.ts";

// /**
//  * [IoDescriptor]
//  * - Mode A (Schema-Strict): FrameDef를 기반으로 키 없이 값만 직렬화 (I/O 최적화)
//  * - Mode B (Schemaless): 키와 값을 함께 직렬화하여 자가 기술적 구조 형성 (IPC/Debug)
//  * * *SAB(SharedArrayBuffer) 환경에서 Zero-Copy로 데이터를 관리합니다.*
//  */
// export class IoDescriptor {
//   // [Internal State]
//   private _viewMap = new Map<string, Uint8Array>(); // Read Cache
//   private _bufferMap = new Map<string, Uint8Array>(); // Write Buffer
//   private _view: DataView;

//   /**
//    * @param _memory 작업을 수행할 메모리 공간 (SAB, Heap)
//    * @param _schema (Optional) 주입 시 'Schema-Strict' 모드로 동작 (PageDescriptor 역할)
//    */
//   constructor(
//     private readonly _memory: Uint8Array,
//     private readonly _schema?: FrameDef,
//   ) {
//     this._view = new DataView(_memory.buffer, _memory.byteOffset, _memory.byteLength);
//   }

//   // ===========================================================================
//   // 📖 READ MODE (Unpack / Parsing)
//   // ===========================================================================

//   /**
//    * [load]
//    * 메모리에 적재된 바이너리를 스캔하여 채널별 뷰(View)를 생성합니다.
//    */
//   public load(): void {
//     let offset = 0;

//     // ---------------------------------------------------------
//     // Mode A: Schema-Strict (Manifest Driven)
//     // 순서: [Len][Body] -> [Len][Body] ... (키 없음, 순서로 식별)
//     // ---------------------------------------------------------
//     if (this._schema) {
//       for (const channel of this._schema.channels) {
//         // 1. Length Reading (4 bytes)
//         // 네트워크 프로토콜은 보통 Little Endian (WASM/x86 Standard) 사용
//         const bodyLen = this._view.getUint32(offset, true);
//         offset += 4;

//         // 2. View Creation (Zero-Copy)
//         // 유효한 데이터가 있는 경우에만 뷰 생성
//         if (bodyLen > 0) {
//           const body = this._memory.subarray(offset, offset + bodyLen);
//           this._viewMap.set(channel.id, body);
//           offset += bodyLen;
//         } else {
//           // 길이가 0이면 맵에 넣지 않거나 빈 배열 처리
//           this._viewMap.set(channel.id, new Uint8Array(0));
//         }
//       }
//       return;
//     }

//     // ---------------------------------------------------------
//     // Mode B: Schemaless (Self-Describing)
//     // 순서: [Count] -> [KeyLen][Key][ValLen][Val] ...
//     // ---------------------------------------------------------
//     const channelCount = this._view.getUint8(offset);
//     offset += 1;

//     const decoder = new TextDecoder();
//     for (let i = 0; i < channelCount; i++) {
//       // Key
//       const keyLen = this._view.getUint8(offset);
//       offset += 1;
//       const key = decoder.decode(this._memory.subarray(offset, offset + keyLen));
//       offset += keyLen;

//       // Value
//       const bodyLen = this._view.getUint32(offset, true);
//       offset += 4;
//       const body = this._memory.subarray(offset, offset + bodyLen);
//       this._viewMap.set(key, body);

//       offset += bodyLen;
//     }
//   }

//   /**
//    * [read]
//    * 특정 채널의 데이터를 읽어옵니다. (Zero-Copy View 반환)
//    */
//   public read(channelId: string): Uint8Array | null {
//     const data = this._viewMap.get(channelId);
//     return (data && data.byteLength > 0) ? data : null;
//   }

//   // ===========================================================================
//   // ✍️ WRITE MODE (Pack / Assembly)
//   // ===========================================================================

//   public write(channelId: string, data: Uint8Array): void {
//     this._bufferMap.set(channelId, data);
//   }

//   /**
//    * [flush]
//    * 버퍼링된 채널들을 메모리(SAB)에 하나의 Frame으로 직렬화합니다.
//    * @returns 작성된 총 바이트 수
//    */
//   public flush(): number {
//     let offset = 0;

//     // ---------------------------------------------------------
//     // Mode A: Schema-Strict (Manifest Driven) - Network Optimized
//     // PageDescriptor의 flush 로직과 동일 (OpCode 제외, 순수 페이로드만)
//     // ---------------------------------------------------------
//     if (this._schema) {
//       for (const channel of this._schema.channels) {
//         // 데이터가 없으면 0바이트 배열로 처리 (순서 유지를 위해 필수)
//         const data = this._bufferMap.get(channel.id) || new Uint8Array(0);

//         // [Length(4)] + [Body(N)] 구조
//         this._view.setUint32(offset, data.byteLength, true);
//         offset += 4;

//         // Body Copy
//         if (data.byteLength > 0) {
//           this._memory.set(data, offset);
//           offset += data.byteLength;
//         }
//       }
//       return offset;
//     }

//     // ---------------------------------------------------------
//     // Mode B: Schemaless (Generic) - Debug/IPC Optimized
//     // ---------------------------------------------------------
//     this._view.setUint8(offset, this._bufferMap.size);
//     offset += 1;

//     const encoder = new TextEncoder();
//     for (const [key, data] of this._bufferMap) {
//       const keyBytes = encoder.encode(key);

//       // Key
//       this._view.setUint8(offset, keyBytes.length);
//       offset += 1;
//       this._memory.set(keyBytes, offset);
//       offset += keyBytes.length;

//       // Value
//       this._view.setUint32(offset, data.byteLength, true);
//       offset += 4;
//       this._memory.set(data, offset);
//       offset += data.byteLength;
//     }

//     return offset;
//   }
// }

// // // @pxl/engine/sys/mm.ts
// // // 메모리 관리자

// // import { IVec } from "@pxl/spec/space/pvf3.ts";

// // /** [MM] Memory Management Interface */
// // export interface MemoryManager {
// //   // 힙 메모리 할당 (mmap)
// //   alloc(size: number): Promise<Uint8Array>;
// //   // 메모리 복사 (memcpy)
// //   copy(dst: Uint8Array, src: Uint8Array): void;

// //   /** * [PT] Page Table
// //    * Spatial Address(IVec) -> Physical Segment(Uint8Array)
// //    */
// //   readonly pt: Map<IVec, Uint8Array>;
// // }

// // 모든 객체는 SAB 안에 있음 (JS/WASM 둘 다 동일하게 참조)
// export type Ptr = number; // byte offset (u32)
// export type Len = number; // byte length (u32)

// export type Slice = { ptr: Ptr; len: Len };

// export interface MemoryManager {
//   alloc(len: Len, align?: number): Slice; // bump/arena
//   free(_slice: Slice): void; // (옵션) or epoch-based
//   view(slice: Slice): Uint8Array; // SAB view 생성 (JS side only)
// }

// // type Ptr = number; // u32 offset
// // type Handle = Ptr; // (원하면 ptr+len인데, 너는 C스럽게 ptr만 선호할 듯)

// // interface Store<K, V> {
// //   get(k: K): Handle | 0;      // 0 = null
// //   put(k: K, v: Handle): void; // v는 이미 SAB 어딘가에 있음
// //   del(k: K): void;
// // }

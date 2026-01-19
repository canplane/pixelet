// /** @xpute/spec/io/manifest.ts */

// /**
//  * [Resource Schema]
//  * 시스템 리소스의 물리적 배치와 전송 전략을 정의하는 범용 표준 명세입니다.
//  */

// // -----------------------------------------------------------------------------
// // 1. Primitive Types
// // -----------------------------------------------------------------------------

// /**
//  * [DataType]
//  * 바이너리 스트림 내의 원시 데이터 타입입니다.
//  * IoDescriptor/DataView가 읽을 때 바이트 길이를 결정합니다.
//  */
// export type DataType = "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "f32" | "f64";

// /**
//  * [LayoutStrategy] (Channel Level)
//  * 수신된 바이너리 데이터를 메모리에 배치/해석하는 알고리즘입니다.
//  * IoDescriptor가 이 힌트를 보고 파싱합니다.
//  */
// export type LayoutStrategy =
//   | "DENSE" // Flat Array (e.g. Elevation). 오프셋 = 인덱스 * 사이즈.
//   | "SPARSE" // 희소 배열. 별도의 인덱스 테이블이 존재함.
//   | "HYPERTREE" // Quadtree/Octree 기반. LOD 처리에 최적화.
//   | "RLE"; // Run-Length Encoding (압축됨).

// // -----------------------------------------------------------------------------
// // 2. Hierarchy Definitions
// // -----------------------------------------------------------------------------

// /**
//  * [Channel Definition]
//  * 가장 작은 단위의 데이터 배열입니다. (예: 높이 맵, 지형 타입)
//  */
// export interface ChannelDef {
//   id: string; // 식별자 (e.g. 'ele', 'land')
//   type: DataType; // 데이터 타입 (e.g. 'i16')
//   layout: LayoutStrategy; // 배치 전략 (e.g. 'DENSE')
// }

// /**
//  * [Frame Definition] (기존 Stream/Packet)
//  * 하나의 물리적 단위(Frame)로 묶이는 채널들의 집합입니다.
//  * IoDescriptor는 이 단위를 기준으로 SAB/Heap 메모리를 구획합니다.
//  */
// export interface FrameDef {
//   id: string; // 프레임 ID (e.g. 'terrain')
//   opCode: number; // [Protocol] 디스패치를 위한 고유 식별자
//   channels: ChannelDef[]; // 내부 데이터 구조
// }

// /**
//  * [Layer Definition]
//  * 동일한 도메인에 속하는 프레임들의 논리적 그룹입니다.
//  */
// export interface LayerDef {
//   id: string; // 레이어 ID (e.g. 'base')
//   frames: FrameDef[]; // 프레임 목록
// }

// /**
//  * [Resource Manifest]
//  * 엔진 모듈이 커널에게 제출하는 데이터 구조 명세서입니다.
//  */
// export interface ResourceManifest {
//   version: string;
//   layers: LayerDef[];
// }

declare module 'three/examples/jsm/utils/BufferGeometryUtils' {
  import { BufferGeometry } from 'three';
  export function mergeGeometries(
    geometries: BufferGeometry[],
    useGroups?: boolean,
  ): BufferGeometry;
  export function mergeVertices(
    geometry: BufferGeometry,
    tolerance?: number,
  ): BufferGeometry;
  // 필요한 다른 함수가 있다면 여기에 추가 정의 가능합니다.
}

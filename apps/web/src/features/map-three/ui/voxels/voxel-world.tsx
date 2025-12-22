// (차후) InstancedMesh를 관리할 컨테이너

// (예시)
// import * as THREE from 'three';
// import { useRef, useLayoutEffect } from 'react';
// import { projection } from '@/shared/lib/geo/projection';

// export const VoxelWorld = ({ data }: { data: VoxelData[] }) => {
//   const meshRef = useRef<THREE.InstancedMesh>(null);
//   const { VOXEL_SIZE } = projection;

//   useLayoutEffect(() => {
//     if (!meshRef.current) return;

//     const tempObject = new THREE.Object3D();
//     data.forEach((voxel, i) => {
//       const world = projection.gridToWorld(voxel.gx, voxel.gy, voxel.gz);
//       // Floating Origin은 부모 그룹에서 처리하거나 여기서 Offset 차감
//       tempObject.position.set(world.x, world.y, world.z);
//       tempObject.updateMatrix();
//       meshRef.current!.setMatrixAt(i, tempObject.matrix);
//     });

//     meshRef.current.instanceMatrix.needsUpdate = true;
//   }, [data]);

//   return (
//     <instancedMesh ref={meshRef} args={[null, null, data.length]}>
//       <boxGeometry args={[VOXEL_SIZE, VOXEL_SIZE, VOXEL_SIZE]} />
//       <meshStandardMaterial />
//     </instancedMesh>
//   );
// };

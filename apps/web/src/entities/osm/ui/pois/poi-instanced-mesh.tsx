import React, { useLayoutEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapStore } from '../../model/map-store';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';
import { CoordOps, to3D } from '@pixelet/shared/shared/model/coords';
import { getTerrainAltitude } from './poi-transform'; // 🎯 Local Import

// 🎯 [Instance-Config]
const TEMP_OBJ = new THREE.Object3D();
const COLOR_NIGHT = new THREE.Color('#FFF59D'); // 연한 노란색

interface PoiInstancedMeshProps {
  pois: MapPoi[]; // 10,000개 이상의 POI 데이터
}

/**
 * --------------------------------------------------------------------------
 * [Shader Strategy: Relative To Eye (RTE)]
 * --------------------------------------------------------------------------
 * @why
 * 지구가 매우 크기 때문에 절대 좌표(AbsPos)를 float32(GPU)로 그대로 넘기면
 * 소수점 정밀도 부족으로 인해 물체가 덜덜 떨리는(Jittering) 현상이 발생합니다.
 *
 * @how
 * 1. [CPU/JS]: 64-bit Double 정밀도를 활용하여 (절대좌표 - 기준점)을 계산해 Attribute로 넘깁니다.
 * 이 값은 상대적으로 매우 작으므로 float32로 변환해도 정보 손실이 없습니다.
 * 2. [GPU/Shader]: (현재 오리진 - 기준점)을 Uniform으로 받아,
 * Attribute에서 Uniform을 빼는 단순 연산으로 '현재 카메라 기준 위치'를 복원합니다.
 */
const PoiShaderMaterial = new THREE.ShaderMaterial({
  uniforms: {
    uOriginDelta: { value: new THREE.Vector3(0, 0, 0) }, // (Current - Anchor)
    uColor: { value: COLOR_NIGHT },
    uCameraPos: { value: new THREE.Vector3() }, // LOD 계산용
  },
  vertexShader: `
    uniform vec3 uOriginDelta;
    uniform vec3 uCameraPos;
    attribute vec3 aRelativePos; 

    varying vec2 vUv;
    varying float vVisible;

    void main() {
      vUv = uv;

      // 1. [RTE Logic] 좌표 복원 (High Precision)
      vec3 worldPos = aRelativePos - uOriginDelta;

      // 2. [LOD Logic] 거리 기반 필터링
      float distSq = dot(worldPos - uCameraPos, worldPos - uCameraPos);
      
      bool isNear = distSq < 2500.0;     // 50m
      bool isFar = distSq > 12250000.0;  // 3.5km
      
      vVisible = (isNear || isFar) ? 0.0 : 1.0;

      // 3. [Billboarding]
      vec3 viewPos = (modelViewMatrix * vec4(worldPos, 1.0)).xyz;
      
      vec3 finalPos = viewPos;
      finalPos.x += position.x * 2.0 * vVisible; // Scale 2.0
      finalPos.y += position.y * 2.0 * vVisible;

      gl_Position = projectionMatrix * vec4(finalPos, 1.0);
    }
  `,
  fragmentShader: `
    uniform vec3 uColor;
    varying vec2 vUv;
    varying float vVisible;

    void main() {
      if (vVisible < 0.5) discard;

      // 4. [Circle Drawing]
      vec2 center = vec2(0.5);
      float dist = distance(vUv, center);
      
      if (dist > 0.5) discard;

      float alpha = 1.0 - smoothstep(0.45, 0.5, dist);

      gl_FragColor = vec4(uColor, alpha * 0.8);
    }
  `,
  transparent: true,
  depthWrite: false,
  side: THREE.DoubleSide,
});

export const PoiInstancedMesh = React.memo(
  ({ pois }: PoiInstancedMeshProps) => {
    const meshRef = useRef<THREE.InstancedMesh>(null);
    const tiles = useMapStore((state) => state.tiles);

    // 🎯 [Anchor Origin Snapshot]
    const anchorOriginRef = useRef(useMapStore.getState().origin);

    /**
     * 🎯 [CPU Optimization: O(N) -> O(1)]
     */
    const { attributes, instanceCount } = useMemo(() => {
      const posArray = new Float32Array(pois.length * 3);
      const anchor = anchorOriginRef.current; // 기준점

      pois.forEach((poi, i) => {
        // 1. 높이 계산 (Terrain Snap)
        const h = getTerrainAltitude(
          CoordOps.add(poi.center, VOXEL_CONFIG.HALF),
          tiles,
        );

        // 2. [RTE Core] Anchor 기준 상대 좌표 계산 (Double Precision)
        const absPos = VOXEL_CONFIG.toCenter(poi.center);
        const relativeToAnchor = CoordOps.sub(absPos, anchor);

        // 3. Three.js 좌표계 변환
        const relMeterPos = to3D(relativeToAnchor, h + 1.5);

        posArray[i * 3] = relMeterPos.x;
        posArray[i * 3 + 1] = relMeterPos.z;
        posArray[i * 3 + 2] = -relMeterPos.y;
      });

      return {
        attributes: posArray,
        instanceCount: pois.length,
      };
    }, [pois, tiles]);

    // 🎯 [Attribute Injection]
    useLayoutEffect(() => {
      if (!meshRef.current) return;

      meshRef.current.geometry.setAttribute(
        'aRelativePos',
        new THREE.InstancedBufferAttribute(attributes, 3),
      );

      // Reset standard matrices
      for (let i = 0; i < instanceCount; i++) {
        TEMP_OBJ.position.set(0, 0, 0);
        TEMP_OBJ.scale.setScalar(1);
        TEMP_OBJ.updateMatrix();
        meshRef.current.setMatrixAt(i, TEMP_OBJ.matrix);
      }
      meshRef.current.instanceMatrix.needsUpdate = true;
    }, [attributes, instanceCount]);

    /**
     * [Frame-Loop: GPU Uniform Update]
     */
    useFrame((state) => {
      if (!meshRef.current) return;

      const currentOrigin = useMapStore.getState().origin;
      const anchor = anchorOriginRef.current;

      // 1. [RTE Update] Delta Calculation
      const diffX = currentOrigin.x - anchor.x;
      const diffY = currentOrigin.y - anchor.y;

      // Swap Y/Z for Three.js
      PoiShaderMaterial.uniforms.uOriginDelta.value.set(diffX, 0, -diffY);
      PoiShaderMaterial.uniforms.uCameraPos.value.copy(state.camera.position);
    });

    return (
      <instancedMesh
        ref={meshRef}
        args={[undefined, undefined, instanceCount]}
        frustumCulled={false}
      >
        <planeGeometry args={[1, 1]} />
        <primitive object={PoiShaderMaterial} attach="material" />
      </instancedMesh>
    );
  },
);

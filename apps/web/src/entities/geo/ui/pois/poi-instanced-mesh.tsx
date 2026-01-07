import React, { useLayoutEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from '../../model/map-store';
import { useTileStore } from '../../model/tile-store';
import { toCenter } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import type { MapPoi } from '@pixelet/core/entities/geo/model/osm-feature';
import { CoordOps, to3D } from '@pixelet/core/shared/model/coords';
import { useTerrainAltitude } from '@/entities/geo/lib/use-terrain-altitude';

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
 * 1. [CPU/Anchor]: 데이터를 굽는 시점의 원점(Anchor)을 기준으로 상대 좌표를 계산합니다.
 * - `localPos = AbsolutePos - Anchor`
 * - 이 값은 Double Precision(JS)으로 계산되어 float32 배열에 담깁니다.
 *
 * 2. [GPU/Delta]: 렌더링 시점의 원점(FloatingOrigin)과 Anchor의 차이를 보정합니다.
 * - `uOriginDelta = CurrentFloatingOrigin - Anchor`
 * - VertexShader: `gl_Position = localPos - uOriginDelta`
 * - 결과적으로 `AbsolutePos - CurrentFloatingOrigin`이 되어 정밀도가 유지됩니다.
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
      // aRelativePos는 Anchor 기준, uOriginDelta는 (Current - Anchor) 이므로
      // 식: (Abs - Anchor) - (Current - Anchor) = Abs - Current (순수 로컬 좌표)
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

export const PoiInstancedMesh = React.memo(({ pois }: PoiInstancedMeshProps) => {
  const { getTerrainAltitude } = useTerrainAltitude();

  const meshRef = useRef<THREE.InstancedMesh>(null);

  // 🎯 [Re-Anchoring Trigger]
  // 원점이 바뀌면(Rebase) 데이터를 다시 구워야 정밀도가 유지됩니다.
  // 이를 위해 스토어의 값을 구독합니다.
  const currentFloatingOrigin = useMapStore(useShallow((state) => state.floatingOrigin));

  /**
   * ⚓ [Anchor Ref]
   * 쉐이더(GPU)에게 "이 데이터는 이 원점을 기준으로 구워졌어"라고 알려주기 위한
   * '과거의 원점' 기록용 레퍼런스입니다.
   */
  const anchorRef = useRef(currentFloatingOrigin);

  /**
   * 🎯 [CPU Optimization: O(N) -> O(1)]
   * Floating Origin이 변경될 때마다(의존성 배열) 데이터를 재계산(Re-Baking)합니다.
   */
  const { attributes, instanceCount } = useMemo(() => {
    const posArray = new Float32Array(pois.length * 3);

    // 1. [Snapshot] 계산을 시작하는 이 시점의 원점을 'Anchor'로 확정합니다.
    const anchor = currentFloatingOrigin;
    anchorRef.current = anchor; // Ref 동기화 (useFrame용)

    pois.forEach((poi, i) => {
      // 2. 높이 계산 (Terrain Snap)
      const gh = getTerrainAltitude(toCenter(poi.center));

      // 3. [RTE Core] Anchor 기준 상대 좌표 계산 (Double Precision)
      // 여기서 (절대좌표 - Anchor)를 미리 계산해둠으로써
      // 이후 Anchor가 얼마나 멀어지든 이 값은 작은 상태로 유지됩니다.
      const absPos = toCenter(poi.center);
      let localPos = CoordOps.sub(absPos, anchor);

      // 4. Three.js 좌표계 변환
      localPos = to3D(localPos, gh + 1.5);

      posArray[i * 3] = localPos.x;
      posArray[i * 3 + 1] = localPos.z;
      posArray[i * 3 + 2] = -localPos.y;
    });

    return {
      attributes: posArray,
      instanceCount: pois.length,
    };
  }, [pois, currentFloatingOrigin, getTerrainAltitude]); // ⚠️ 중요: 원점이 바뀌면 다시 굽는다.

  // 🎯 [Attribute Injection]
  useLayoutEffect(() => {
    if (!meshRef.current) return;

    meshRef.current.geometry.setAttribute('aRelativePos', new THREE.InstancedBufferAttribute(attributes, 3));

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
   * 매 프레임 변화하는 '현재 원점'과 데이터가 구워진 '과거 원점(Anchor)' 사이의
   * 차이(Delta)를 계산하여 쉐이더에 주입합니다.
   */
  useFrame((state) => {
    if (!meshRef.current) return;

    // 실시간 원점 (Present)
    const realtimeOrigin = useMapStore.getState().floatingOrigin;
    // 데이터가 구워진 시점의 원점 (Past)
    const anchor = anchorRef.current;

    // 1. [RTE Update] Delta Calculation
    // 쉐이더 수식: (Abs - Anchor) - (Realtime - Anchor) = Abs - Realtime
    // 따라서 Delta = Realtime - Anchor
    const diffX = realtimeOrigin.x - anchor.x;
    const diffY = realtimeOrigin.y - anchor.y;

    // Swap Y/Z for Three.js Coordinate System
    PoiShaderMaterial.uniforms.uOriginDelta.value.set(diffX, 0, -diffY);
    PoiShaderMaterial.uniforms.uCameraPos.value.copy(state.camera.position);
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, instanceCount]} frustumCulled={false}>
      <planeGeometry args={[1, 1]} />
      <primitive object={PoiShaderMaterial} attach="material" />
    </instancedMesh>
  );
});

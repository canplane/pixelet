import React, { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import type { MapInfra } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';
import { useMapNavigation } from '../../model/use-map-navigation';

export const OsmInfraItem = React.memo(({ infra }: { infra: MapInfra }) => {
  const objectRef = useRef<THREE.Object3D>(null);

  // 🚀 [Floating Origin] 전역 오프셋 구독
  // 이 값이 변하면 OsmInfraItem이 리렌더링되며 위치를 재계산합니다.
  const { originOffset } = useMapNavigation();

  const isMeshType =
    (infra.type === 'building' ||
      infra.type === 'water' ||
      infra.type === 'landuse') &&
    infra.shape;

  /**
   * 1. Geometry 생성 (기존과 동일)
   */
  const geometry = useMemo(() => {
    if (isMeshType && infra.shape) {
      const depth = infra.type === 'building' ? infra.height : 0.1;
      const geo = new THREE.ExtrudeGeometry(infra.shape, {
        depth,
        bevelEnabled: false,
      });
      geo.rotateX(-Math.PI / 2);
      geo.computeBoundingSphere();
      return geo;
    }
    const geo = new THREE.BufferGeometry().setFromPoints(infra.points);
    geo.computeBoundingSphere();
    return geo;
  }, [infra, isMeshType]);

  /**
   * 2. Material 생성
   * [디자인] 건물 재질에 약간의 금속 질감과 거칠기를 조절하여 플루언트한 느낌을 줍니다.
   */
  const material = useMemo(() => {
    const baseOptions = {
      transparent: true,
      opacity: 0,
      depthWrite: infra.type === 'building',
    };

    switch (infra.type) {
      case 'building':
        return new THREE.MeshStandardMaterial({
          ...baseOptions,
          // 🚀 1. 색상을 더 어둡고 푸른 기가 도는 회색으로 (하얗게 타는 것 방지)
          color: '#4a4e54',
          // 🚀 2. 거칠기를 높여 반사광을 분산시킴 (눈부심 방지)
          roughness: 0.7,
          metalness: 0.2,
          // 🚀 3. 평면을 더 부드럽게 보이게 함
          flatShading: false,
        });
      case 'water':
        return new THREE.MeshStandardMaterial({
          ...baseOptions,
          color: '#4da3ff', // 조금 더 맑은 물 색상
          roughness: 0.1,
          metalness: 0.5,
        });
      case 'road':
        return new THREE.LineBasicMaterial({
          ...baseOptions,
          color: '#ffffff', // 도로는 흰색 실선 느낌
          opacity: 0.3,
        });
      default:
        return new THREE.LineBasicMaterial({
          ...baseOptions,
          color: '#999999',
        });
    }
  }, [infra.type]);

  /**
   * 2. Object 생성 (최초 1회 혹은 데이터 변경시에만)
   * 🚀 position 설정을 여기서 빼고 useEffect로 넘깁니다.
   */
  const object = useMemo(() => {
    const obj = isMeshType
      ? new THREE.Mesh(geometry, material)
      : new THREE.Line(geometry, material);
    return obj;
  }, [geometry, material, isMeshType]);

  /**
   * 3. [핵심] 위치 보정 전용 Effect
   * 🚀 Re-render 없이 Three.js 객체의 위치만 직접 수정합니다. (성능 최적화)
   */
  useEffect(() => {
    if (!objectRef.current) return;

    // 리렌더링 없이 행렬값만 즉시 수정하여 성능 부하를 최소화합니다.
    objectRef.current.position.set(
      infra.center.x - originOffset.x,
      infra.center.y - originOffset.y,
      infra.center.z - originOffset.z,
    );
  }, [originOffset, infra.center]); // 오프셋이 변할 때만 딱 실행됨

  /**
   * 4. 거리 기반 페이드 애니메이션 (기존 로직 유지)
   */
  useFrame((state, delta) => {
    if (!objectRef.current) return;
    const obj = objectRef.current;
    const mat = (obj as any).material as THREE.Material;

    // 현재 카메라와 이 물체의 거리를 계산 (둘 다 로컬 좌표계이므로 계산이 정확함)
    const distance = state.camera.position.distanceTo(obj.position);

    let maxOpacity = 0.8;
    if (infra.type === 'road') maxOpacity = 0.4;

    let targetOpacity = maxOpacity;

    if (distance > THREE_CONFIG.RENDERING.FADE_START) {
      targetOpacity = THREE.MathUtils.mapLinear(
        distance,
        THREE_CONFIG.RENDERING.FADE_START,
        THREE_CONFIG.RENDERING.FADE_END,
        maxOpacity,
        0,
      );
    }

    mat.opacity = THREE.MathUtils.lerp(
      mat.opacity,
      Math.max(0, targetOpacity),
      delta * 4, // 🚀 조금 더 빠르게 반응하도록 lerp 속도 조절
    );
    obj.visible = mat.opacity > 0.01;
  });

  return <primitive ref={objectRef} object={object} />;
});

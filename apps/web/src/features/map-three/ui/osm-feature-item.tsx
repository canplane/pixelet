import React, { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { mapConverter } from '@/shared/lib/geo/map-converter'; // 프로젝트 내 변환 유틸
import type { ParsedFeature } from '../lib/osm-parser';
import { THREE_CONFIG } from '../model/three-config';

/**
 * 개별 지형지물을 그리는 내부 컴포넌트
 * 여기서 Geometry를 메모이제이션하고 경계를 계산하여 렌더링 누락을 방지합니다.
 */
export const OsmFeatureItem = React.memo(
  ({ feature }: { feature: ParsedFeature }) => {
    const isBuilding = feature.type === 'building' && feature.shape;
    const objectRef = useRef<THREE.Object3D>(null);

    // 1. Geometry 생성 (기존 로직 유지)
    const geometry = useMemo(() => {
      if (isBuilding && feature.shape) {
        const depth = mapConverter.metersToUnits(feature.height);
        const geo = new THREE.ExtrudeGeometry(feature.shape, {
          depth,
          bevelEnabled: false,
        });
        geo.rotateX(-Math.PI / 2);
        geo.computeBoundingSphere();
        return geo;
      }
      const geo = new THREE.BufferGeometry().setFromPoints(feature.points);
      geo.computeBoundingSphere();
      return geo;
    }, [feature, isBuilding]);

    // 2. Material 생성 (초기 투명도 0 설정)
    const material = useMemo(() => {
      if (isBuilding) {
        return new THREE.MeshStandardMaterial({
          color: '#666666',
          transparent: true,
          opacity: 0,
          depthWrite: true, // 건물은 깊이 테스트 기록 필요
        });
      }

      return new THREE.LineBasicMaterial({
        color: '#00ff00',
        transparent: true,
        opacity: 0,
        depthWrite: false, // 도로는 바닥에 깔리도록 기록 안 함
      });
    }, [isBuilding]);

    // 3. Object 생성
    const object = useMemo(() => {
      return isBuilding
        ? new THREE.Mesh(geometry, material)
        : new THREE.Line(geometry, material);
    }, [geometry, material, isBuilding]);

    // 4. [핵심] 통합 페이드 애니메이션 로직
    useFrame((state, delta) => {
      if (!objectRef.current) return;

      const obj = objectRef.current;
      const mat = (obj as any).material as THREE.Material;

      // 현재 카메라와의 거리 측정
      const distance = state.camera.position.distanceTo(obj.position);

      // 목표 투명도 결정 (기본값)
      let targetOpacity = isBuilding ? 0.8 : 0.6;

      // A. 거리 기반 페이드 아웃 (멀어질 때)
      if (distance > THREE_CONFIG.RENDERING.FADE_START) {
        targetOpacity = THREE.MathUtils.mapLinear(
          distance,
          THREE_CONFIG.RENDERING.FADE_START,
          THREE_CONFIG.RENDERING.FADE_END,
          targetOpacity,
          0,
        );
      }

      // B. 생성 시 페이드 인 및 거리 기반 페이드 아웃 통합 적용 (Lerp)
      // 현재 opacity에서 목표 opacity로 부드럽게 이동
      mat.opacity = THREE.MathUtils.lerp(
        mat.opacity,
        Math.max(0, targetOpacity),
        delta * 2,
      );
    });

    return <primitive ref={objectRef} object={object} />;
  },
);

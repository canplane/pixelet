import React, { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import type { ParsedFeature } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';

/**
 * 개별 지형지물(건물, 도로)을 3D 메쉬로 생성하고 렌더링하는 컴포넌트
 */
export const OsmFeatureItem = React.memo(
  ({ feature }: { feature: ParsedFeature }) => {
    const isBuilding = feature.type === 'building' && feature.shape;
    const objectRef = useRef<THREE.Object3D>(null);

    /**
     * [보강] 피처의 중심점(절대 좌표)
     * 이제 parser에서 feature.center로 절대 미터 좌표를 직접 넘겨줍니다.
     */
    const absolutePosition = useMemo(() => {
      // feature.center는 THREE.Vector3 타입이므로 그대로 사용합니다.
      return feature.center;
    }, [feature.center]);

    const geometry = useMemo(() => {
      if (isBuilding && feature.shape) {
        /**
         * 1. Geometry 생성
         * 1유닛이 1미터이므로 feature.height(m)를 그대로 depth로 사용합니다.
         */
        const depth = feature.height;
        const geo = new THREE.ExtrudeGeometry(feature.shape, {
          depth,
          bevelEnabled: false,
        });

        // Extrude는 Z축 방향으로 생성되므로, 지면과 수직이 되도록 X축 회전
        geo.rotateX(-Math.PI / 2);
        geo.computeBoundingSphere();
        return geo;
      }

      // 도로 등 선형 데이터 생성
      // feature.points는 이미 center 대비 상대 좌표로 변환되어 있습니다.
      const geo = new THREE.BufferGeometry().setFromPoints(feature.points);
      geo.computeBoundingSphere();
      return geo;
    }, [feature, isBuilding]);

    /**
     * 2. Material 생성
     * 초기 투명도를 0으로 설정하여 생성 시 부드럽게 나타나도록(Fade-in) 합니다.
     */
    const material = useMemo(() => {
      if (isBuilding) {
        return new THREE.MeshStandardMaterial({
          color: '#666666',
          transparent: true,
          opacity: 0, // 초기값 0 (useFrame에서 페이드인)
          depthWrite: true,
        });
      }

      return new THREE.LineBasicMaterial({
        color: '#00ff00',
        transparent: true,
        opacity: 0,
        depthWrite: false, // 도로는 지면에 밀착되므로 깊이 기록 제외
      });
    }, [isBuilding]);

    /**
     * 3. Object 생성
     */
    const object = useMemo(() => {
      const obj = isBuilding
        ? new THREE.Mesh(geometry, material)
        : new THREE.Line(geometry, material);

      /**
       * [핵심 수정] 객체의 위치를 절대 좌표로 설정합니다.
       * 이렇게 해야 카메라와의 거리 계산(distanceTo)이 정확해집니다.
       * geometry는 이미 상대 좌표이므로, 이 position 설정으로 정확한 위치에 안착합니다.
       */
      obj.position.copy(absolutePosition);

      return obj;
    }, [geometry, material, isBuilding, absolutePosition]);

    /**
     * 4. [최적화] 통합 페이드 애니메이션
     * 카메라와의 거리에 따라 자동으로 안개 속으로 사라지거나 나타나는 효과를 줍니다.
     */
    useFrame((state, delta) => {
      if (!objectRef.current) return;

      const obj = objectRef.current;
      const mat = (obj as any).material as THREE.Material;

      // 이제 obj.position이 수백만 단위이므로 카메라(state.camera.position)와의 거리 계산이 가능합니다.
      const distance = state.camera.position.distanceTo(obj.position);

      let targetOpacity = isBuilding ? 0.8 : 0.6;

      // 거리 기반 페이드 로직 (THREE_CONFIG 값을 확인하세요)
      if (distance > THREE_CONFIG.RENDERING.FADE_START) {
        targetOpacity = THREE.MathUtils.mapLinear(
          distance,
          THREE_CONFIG.RENDERING.FADE_START,
          THREE_CONFIG.RENDERING.FADE_END,
          targetOpacity,
          0,
        );
      }

      // 최종 투명도 보간 (Lerp)
      mat.opacity = THREE.MathUtils.lerp(
        mat.opacity,
        Math.max(0, targetOpacity),
        delta * 2,
      );

      // 최적화: 투명도가 거의 0이면 객체를 숨깁니다.
      obj.visible = mat.opacity > 0.01;
    });

    return <primitive ref={objectRef} object={object} />;
  },
);

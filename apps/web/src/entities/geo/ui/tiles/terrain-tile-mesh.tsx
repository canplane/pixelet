/**
 * @file TerrainTileMesh.tsx
 * [Layer: VIEW] SparseQuadtree 데이터를 기반으로 지형 타일을 렌더링하는 컴포넌트입니다.
 * [Architecture]
 * - SparseQuadtree.flatten()을 사용하여 렌더링에 필요한 Dense 데이터를 추출합니다.
 * - 추출된 데이터를 DataTexture로 변환하여 Custom Shader Material에 주입합니다.
 * - [Refactor] useCameraVisibility 브릿지를 통해 고도 인식형 LOD 시스템을 구현합니다.
 */

import React, { useState, useLayoutEffect, useMemo, useRef, useEffect } from 'react';
import * as THREE from 'three';
import { useThree, useFrame } from '@react-three/fiber';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { ColorOps } from '@pixelet/core/entities/cell/model/utils';
import { useRelativePosition } from '../../lib/use-coordinate-converter';
import type { Tile, TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';
import { TILE_SURFACE_KEYS } from '@pixelet/core/entities/geo/model/tiles';
import { THREE_CONFIG } from '../../config/three-config';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import { getTileBbox } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { createChannelTexture, createElevationTexture } from '../../lib/render/terrain/texture-factory';
import { createTileMaterial } from '../../lib/render/terrain/tile-material';
import { useCameraVisibility } from '@/entities/geo/model/use-camera-visibility';

const { CELL_SIZE, TILE_SIZE, TILE_RES } = GEO_CONSTANTS;

const SHARED_GEOMETRY = new THREE.BoxGeometry(1, 1, 1);
const TILE_RES_VEC2 = new THREE.Vector2(TILE_RES, TILE_RES);

const GRID_COLOR_TARGET = '#000000';
const GRID_CONFIG = {
  uBorderColor: new THREE.Color(ColorOps.isValidHex(GRID_COLOR_TARGET) ? GRID_COLOR_TARGET : '#000000'),
  uBorderOpacity: 0.1,
};

interface TerrainTileMeshProps {
  tile: Tile;
}

export const TerrainTileMesh = React.memo(({ tile }: TerrainTileMeshProps) => {
  const { coord, base } = tile;
  const { surface: surfaceData, ele: eleData } = base;

  const [currentLod, setCurrentLod] = useState<number>(1); // 🎯 초기값 1: 로딩 즉시 해상도 확보
  const { gl } = useThree();

  /**
   * 🎯 [Bridge] 카메라 가시성 브릿지 구독
   * 지표면 앵커와 카메라의 절대 좌표를 기반으로 이 타일의 LOD를 결정합니다.
   */
  const { getVisibilityGuide } = useCameraVisibility();

  /** 🎯 타일의 절대 좌표 경계 (LOD 판정용) */
  const absoluteBbox = useMemo(() => getTileBbox(coord), [coord]);

  /** 🎯 타일의 절대 중심점 (상대 위치 변환용) */
  const absoluteCenter = useMemo(
    () => ({
      x: absoluteBbox.minX + TILE_SIZE / 2,
      y: absoluteBbox.minY + TILE_SIZE / 2,
    }),
    [absoluteBbox],
  );

  const [surfaceTextures, setSurfaceTextures] = useState<Record<TileSurfaceKey, THREE.DataTexture> | null>(null);
  const [elevationTexture, setElevationTexture] = useState<THREE.DataTexture | null>(null);

  const frameCounter = useRef(Math.floor(Math.random() * 20));

  /** 🎯 Floating Origin을 고려한 카메라 기준 상대 위치 계산 */
  const relativePosition = useRelativePosition({ ...absoluteCenter, z: 0 });

  /**
   * [Texture Factory]
   * SparseQuadtree 데이터를 GPU 텍스처로 변환하는 비동기 프로세스입니다.
   * isMounted 패턴으로 메모리 누수를 방지합니다.
   */
  useEffect(() => {
    let isMounted = true;
    const prepareTextures = async () => {
      const surfacePromises = TILE_SURFACE_KEYS.map(async (key) => {
        const tree = surfaceData[key];
        const flatData = tree.flatten();
        return { key, tex: createChannelTexture(flatData, gl as THREE.WebGLRenderer) };
      });

      const flatEleData = eleData.flatten();
      const surfaceResults = await Promise.all(surfacePromises);

      if (!isMounted) return;

      const texMap = {} as Record<TileSurfaceKey, THREE.DataTexture>;
      surfaceResults.forEach(({ key, tex }) => {
        texMap[key] = tex;
      });

      const elTex = createElevationTexture(flatEleData);
      setSurfaceTextures(texMap);
      setElevationTexture(elTex);
    };

    prepareTextures();
    return () => {
      isMounted = false;
      // 🎯 리소스 해제: WebGL 메모리 관리
      if (surfaceTextures) Object.values(surfaceTextures).forEach((t) => t.dispose());
      if (elevationTexture) elevationTexture.dispose();
    };
  }, [surfaceData, eleData, gl]);

  /**
   * [LOD Update Loop]
   * 🎯 브릿지에서 결정된 가이드(LOD, 인스턴스 개수)를 수신합니다.
   * 15프레임마다 체크하여 성능 최적화를 꾀합니다.
   */
  useFrame(() => {
    frameCounter.current += 1;
    if (frameCounter.current % 15 === 0) {
      // 🚀 훅 내부의 calculateInstanceCount가 계산한 결과값을 포함한 guide를 받습니다.
      const guide = getVisibilityGuide(absoluteBbox, currentLod);

      if (guide.lod !== currentLod) {
        setCurrentLod(guide.lod);
        // 여기서 guide.instanceCount는 아래 useMemo에서 다시 계산되어 정합성을 유지합니다.
      }
    }
  });

  /**
   * 🎯 [Refactor] 인스턴스 개수 결정
   * 훅 안에 로직이 있지만, 뷰가 리렌더링될 때 currentLod 상태에 맞춰
   * '동일한 규칙'으로 개수를 보장받아야 합니다.
   * 훅에서 보낸 규칙(2의 배수)을 그대로 따르기 위해 다시 호출하거나
   * 훅에서 내려준 값을 상태로 저장해도 됩니다.
   */
  const instanceCount = useMemo(() => {
    // 💡 훅 내부에 있는 로직과 동일하게 2의 배수를 보장합니다.
    const side = Math.floor(TILE_RES / currentLod);
    return side * side;
  }, [currentLod]);

  /** 🎯 Custom Shader Material 생성 및 유니폼 주입 */
  const material = useMemo(() => {
    if (!surfaceTextures || !elevationTexture) return null;
    return createTileMaterial({
      channelTextures: surfaceTextures,
      elevationTexture: elevationTexture,
      uResolution: TILE_RES_VEC2,
      uVoxelSize: CELL_SIZE, // 1.0m 셀 단위
      uLodScale: currentLod,
      gridConfig: GRID_CONFIG,
    });
  }, [surfaceTextures, elevationTexture, currentLod]);

  /** 🛡️ Cleanup: 머티리얼 인스턴스 해제 */
  useEffect(() => {
    return () => material?.dispose();
  }, [material]);

  const meshRef = useRef<THREE.InstancedMesh>(null);

  /**
   * 🎯 [Fix] 카메라 근접 시 타일 소실 방지
   * Floating Origin 좌표계와 인스턴스 셰이더 변위 간의 바운딩 볼륨 불일치를 해결하기 위해
   * 엔진 레벨의 Frustum Culling을 비활성화하고 브릿지의 논리적 컬링에 위임합니다.
   */
  useLayoutEffect(() => {
    if (meshRef.current) {
      meshRef.current.frustumCulled = false;
    }
  }, [currentLod]); // LOD 전환 시 새로 생성되는 인스턴스에 대해 설정 유지

  // 🎯 데이터(텍스처/머티리얼) 준비 전까지는 렌더링하지 않음
  if (!material) return null;

  return (
    <instancedMesh
      ref={meshRef}
      // 🎯 React의 효율적인 재조정을 위해 LOD 레벨별로 별도 인스턴스 취급
      key={`tile-${currentLod}`}
      args={[SHARED_GEOMETRY, undefined, instanceCount]}
      position={toThreeCoordWithOffset(relativePosition, -0.01)}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      /** * 🎯 [Decision] 엔진 기본 컬링 대신 브릿지 기반 논리 컬링 채택
       * 좌표 정밀도 오차로 인한 소실 방지를 위해 명시적으로 비활성화합니다.
       */
      frustumCulled={false}
    >
      <primitive object={material} attach="material" />
    </instancedMesh>
  );
});

// React.memo를 통해 불필요한 리렌더링 방지

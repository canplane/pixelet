/**
 * @file use-camera-visibility.ts
 * [Bridge] 카메라의 절대 좌표와 지표면 기준 상대 고도를 통합 관리하여 가시성 가이드를 제공합니다.
 */

import { useCallback, useMemo } from 'react';
import * as THREE from 'three';
import { useThree } from '@react-three/fiber';
import { useMapNavigation } from './use-map-navigation';
import { useMapCenter } from './use-map-center';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';

const { TILE_RES } = GEO_CONSTANTS;

// 🎯 환경에 따른 스케일 팩터 결정 (데스크톱: 1.0, 모바일: 0.5)
const isMobile = /Mobi|Android|iPhone/i.test(navigator.userAgent);
const SCALE_FACTOR = isMobile ? 0.5 : 1.0;

// ----------------------------------------------------------------------
// 🎯 [LOD Configuration] 2의 배수 및 타일 크기(1024) 기반 정렬
// ----------------------------------------------------------------------
const LOD_MARGIN = 1.25;

const REF_DISTANCES = {
  // L1: 약 512m. 내 주변 8~12장 타일이 초고정밀. (가장 쾌적함)
  L1: 256 * 2,
  // L2: 약 1km.
  L2: 256 * 4,
  // L4: 약 2km.
  L4: 256 * 8,
  // L8: 약 4km. (LOAD_RADIUS 16 지점)
  L8: 256 * 16,
  // L16: 약 8km.
  L16: 256 * 32,
  // L32: 그 너머 지평선.
  L32: Infinity,
};

/** 거리에 따른 LOD 레벨 계산 (Hysteresis 정밀 캘리브레이션) */
export const calculateLodFromDistance = (distance: number, currentLod: number): number => {
  // 🚀 부드러운 전환 핵심: 낮은 해상도에서 높은 해상도로 올 때는 마진(1.25)을 적용해 더 가까이 와야 전환됨
  const getThreshold = (refDist: number) => refDist * SCALE_FACTOR * (currentLod > 1 ? 1 / LOD_MARGIN : 1.0);

  if (distance < getThreshold(REF_DISTANCES.L1)) return 1;
  if (distance < getThreshold(REF_DISTANCES.L2)) return 2;
  if (distance < getThreshold(REF_DISTANCES.L4)) return 4;
  if (distance < getThreshold(REF_DISTANCES.L8)) return 8;
  if (distance < getThreshold(REF_DISTANCES.L16)) return 16;
  return 32;
};

/** 🎯 인스턴스 개수를 2의 배수로 강제하는 계산 */
const calculateInstanceCount = (res: number, lod: number) => {
  const side = Math.floor(res / lod);
  return side * side;
};

export function useCameraVisibility() {
  const { camera } = useThree();
  const { center } = useMapNavigation();
  const groundCenter = useMapCenter();

  const absoluteCameraPos = useMemo(() => {
    return new THREE.Vector3(center.x + camera.position.x, center.y + camera.position.y, camera.position.z);
  }, [center.x, center.y, camera.position.x, camera.position.y, camera.position.z]);

  /**
   * 🎯 [Unified Bridge API] 타일별 가시성 가이드 하달
   * LOD와 인스턴스 개수를 원자적으로 결정하여 반환합니다.
   */
  const getVisibilityGuide = useCallback(
    (bbox: { minX: number; maxX: number; minY: number; maxY: number }, currentLod: number) => {
      // 1. Clamping 기반 최단 거리 계산
      const closestX = Math.max(bbox.minX, Math.min(absoluteCameraPos.x, bbox.maxX));
      const closestY = Math.max(bbox.minY, Math.min(absoluteCameraPos.y, bbox.maxY));

      const dx = absoluteCameraPos.x - closestX;
      const dy = absoluteCameraPos.y - closestY;
      const dz = absoluteCameraPos.z;

      const distance = Math.sqrt(dx * dx + dy * dy + dz * dz);

      // 2. 결정: LOD 및 인스턴스 개수
      const nextLod = calculateLodFromDistance(distance, currentLod);
      const instanceCount = calculateInstanceCount(TILE_RES, nextLod);

      return { lod: nextLod, instanceCount };
    },
    [absoluteCameraPos],
  );

  return {
    getVisibilityGuide,
    absoluteCameraPos,
    groundCenter,
  };
}

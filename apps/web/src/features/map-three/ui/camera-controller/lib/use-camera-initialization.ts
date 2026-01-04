import { useEffect, useLayoutEffect } from 'react';
import { useThree } from '@react-three/fiber';
import { THREE_CONFIG } from '@/entities/geo/config/three-config';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';

const { FAR_CLIP } = THREE_CONFIG.RENDERING;

export function useCameraInitialization() {
  const { center, updateCenter, devicePos } = useMapNavigation();

  const { camera } = useThree();

  // 1. [Camera-Config] 카메라 투영 행렬 설정
  useEffect((): void => {
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  // 2. [Initial-Trigger] 세상의 첫 위치 결정
  useLayoutEffect(() => {
    if (!center && devicePos) {
      updateCenter(devicePos);
    }
  }, [center, devicePos]);
}

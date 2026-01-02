import * as THREE from 'three';
import { OrbitControls } from '@react-three/drei';

import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { useCameraKernel } from '@/features/map-three/ui/camera-controller/lib/use-camera-kernel';

const { MIN_DISTANCE, MAX_DISTANCE } = THREE_CONFIG.CAMERA;

export function MapCameraController() {
  const { setCameraControlMode } = useMapNavigation();

  const { controlsRef, onInteractionStart, onInteractionEnd } =
    useCameraKernel();

  return (
    <OrbitControls
      ref={controlsRef}
      makeDefault
      mouseButtons={{
        LEFT: THREE.MOUSE.PAN,
        MIDDLE: THREE.MOUSE.DOLLY,
        RIGHT: THREE.MOUSE.ROTATE,
      }}
      touches={{
        ONE: THREE.TOUCH.PAN,
        TWO: THREE.TOUCH.DOLLY_ROTATE,
      }}
      enableRotate={true}
      rotateSpeed={1}
      enableDamping={true}
      dampingFactor={0.08}
      minDistance={MIN_DISTANCE}
      maxDistance={MAX_DISTANCE}
      screenSpacePanning={false}
      maxPolarAngle={Math.PI / 2.1}
      minPolarAngle={0}
      onStart={(e) => {
        setCameraControlMode('user'); // 🔒 Lock: 외부(Zustand) 명령 차단
        onInteractionStart();
      }}
      onEnd={(e) => {
        onInteractionEnd();
        setCameraControlMode('none'); // 🔓 Unlock: 다시 외부 명령(FlyTo 등) 수용 가능 상태로 전환
      }}
    />
  );
}

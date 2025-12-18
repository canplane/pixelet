import { useState, useMemo, useEffect } from 'react';
import { debounce } from 'lodash';
import { Canvas } from '@react-three/fiber';
import { Grid } from '@react-three/drei';

import { mapConverter } from '@/shared/lib/geo/map-converter';
import { getInitialCameraPosition } from '../lib/camera-utils';
import { THREE_CONFIG } from '../model/three-config';

import { MapCameraController } from '../ui/map-camera-controller';
import { OsmFeatures } from '../ui/osm-features';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';
import { useMapStore } from '@/entities/map/model/map.store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useDeviceOrientation } from '@/entities/device-location/model/use-orientation';
import { ScaleBar } from '@/shared/ui/map-three/scale-bar';
import { Compass } from '@/shared/ui/map-three/compass';

import type { RelativeWorldPos } from '@/shared/types/map';
import { UserMarker } from '../ui/user-marker';

// --- 메인 지도 컴포넌트 ---
export function MapScene() {
  // 1. 기기 실제 GPS 위치
  const { lngLat: deviceLngLat, status } = useDeviceLocationStore();

  const { requestPermission } = useDeviceOrientation();
  const [isPermissionGranted, setIsPermissionGranted] = useState(false);

  const handleEnableCompass = async () => {
    const granted = await requestPermission();
    setIsPermissionGranted(granted);
  };

  // 2. 지도 시스템 상태
  const { origin, setOrigin, project, unproject } = useMapStore();

  // [수정] WorldPos 대신 명확한 RelativeWorldPos 사용 및 초기값 null 허용
  const [targetPos, setTargetPos] = useState<RelativeWorldPos>([0, 0, 0]);

  // camData 상태 업데이트 빈도를 조절하기 위한 최적화
  const [camData, setCamData] = useState({ dist: 100, rot: 0 });

  // 1. 지도가 보고 있는 중심점의 위경도 계산
  const currentViewLngLat = useMemo(() => {
    if (!origin) return null;
    // unproject 내부에서 origin을 사용하는지 확인하세요.
    return unproject(targetPos);
  }, [targetPos, origin, unproject]);

  /**
   * 2. [핵심] API 호출 최적화 (Debounce)
   * 사용자가 지도를 멈추고 0.8초 후에 새로운 데이터를 요청합니다.
   */
  const [fetchCenter, setFetchCenter] = useState<[number, number] | null>(null);

  const debouncedSetCenter = useMemo(
    () => debounce((pos: [number, number]) => setFetchCenter(pos), 800),
    [],
  );

  useEffect(() => {
    if (currentViewLngLat) {
      debouncedSetCenter(currentViewLngLat);
    }
  }, [currentViewLngLat, debouncedSetCenter]);

  // 2. API 호출 (fetchCenter가 바뀔 때만 요청)
  const { osmData } = useMapFeatures('GEOMETRY', fetchCenter);

  // 3. 통합 위치 및 원점 관리 로직
  useEffect(() => {
    if (status !== 'ready' || !deviceLngLat) return;

    // 초기 상태: 원점이 없으면 현재 위치를 원점으로 잡음
    if (!origin) {
      setOrigin(deviceLngLat);
      setTargetPos([0, 0, 0]);
      return;
    }

    // 현재 위치의 상대 좌표 계산
    const relativePos = project(deviceLngLat);
    if (!relativePos) return;

    // [정밀도 가드] 원점에서 너무 멀어졌다면 (예: 5km) 원점 재설정
    // mapConverter를 이용해 가독성 높임
    const limitUnits = mapConverter.metersToUnits(10000);
    const distance = Math.sqrt(relativePos[0] ** 2 + relativePos[2] ** 2);
    if (distance > limitUnits) {
      console.log('📍 새로운 지역 진입: 원점을 갱신합니다.');
      setOrigin(deviceLngLat);
      setTargetPos([0, 0, 0]); // 원점이 내 위치가 되었으므로 0,0,0으로 리셋
      return;
    }

    // [내 위치 추적 모드일 경우 활성화]
    // setTargetPos(relativePos);
  }, [deviceLngLat, origin, status, project, setOrigin]);

  // 4. 카메라 초기 설정
  const initialPos = useMemo(() => {
    // SSR 대응 및 실제 창 높이 반영
    const screenHeight =
      typeof window !== 'undefined' ? window.innerHeight : 800;
    return getInitialCameraPosition(screenHeight);
  }, []);

  // // SCALE_FACTOR가 반영된 그리드 및 테스트 유닛
  // const gridStep = mapConverter.metersToUnits(100); // 100m 당 그리드 한 칸

  const [showBuildings, setShowBuildings] = useState(false);

  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        background: '#111',
        position: 'relative',
      }}
    >
      {/* 권한 미승인 시 보여줄 안내 버튼 */}
      {!isPermissionGranted && (
        <button
          onClick={handleEnableCompass}
          style={{
            position: 'absolute',
            bottom: 100,
            left: '50%',
            transform: 'translateX(-50%)',
            zIndex: 100,
          }}
        >
          🧭 나침반 활성화
        </button>
      )}

      {/* UI 컨트롤 버튼 예시 */}
      <div style={{ position: 'absolute', top: 100, left: 20, zIndex: 100 }}>
        <label style={{ color: 'white' }}>
          <input
            type="checkbox"
            checked={showBuildings}
            onChange={(e) => setShowBuildings(e.target.checked)}
          />
          건물 표시
        </label>
      </div>

      {/* 테스트용 버튼 */}
      <button
        onClick={() => setTargetPos([500, 0, 500])}
        style={{
          position: 'absolute',
          zIndex: 10,
          top: 20,
          left: 20,
          padding: '8px 12px',
        }}
      >
        동북쪽 500m 이동
      </button>

      <Canvas
        dpr={[1, 2]} // 성능 모드 활성화 (안티앨리어싱 등 조절)
        camera={{
          fov: THREE_CONFIG.CAMERA.FOV,
          position: [initialPos.x, initialPos.y, initialPos.z],
          near: THREE_CONFIG.CAMERA.NEAR,
          far: THREE_CONFIG.RENDERING.FAR_CLIP, // FAR 값을 충분히 크게 설정하세요 (예: 5000)
        }}
      >
        {/* 배경색과 일치하는 안개를 추가하면 멀리 있는 건물이 안개 속으로 사라지는 느낌이 납니다 */}
        <fog
          attach="fog"
          args={[
            '#111111',
            THREE_CONFIG.RENDERING.FOG_NEAR,
            THREE_CONFIG.RENDERING.FOG_FAR,
          ]}
        />

        <ambientLight intensity={1.5} />

        {/* 내 위치 마커 추가 */}
        <UserMarker />

        {/* 메인 3D 지도를 제어하는 카메라 컨트롤러 */}
        {/* [수정] targetPos가 있을 때만 렌더링하여 null 참조 에러 방지 */}
        {targetPos && (
          <MapCameraController
            // key를 활용하면 특정 상황(예: 원점 변경)에서만 카메라를 강제 초기화할 수 있습니다.
            key={`camera-${origin?.[0]}`}
            targetPos={targetPos}
            onMapMove={(newPos) => {
              // 부모의 상태는 업데이트하되, 자식에게 다시 내려가서
              // 카메라 위치를 '강제 set'하지 않도록 주의가 필요합니다.
              setTargetPos(newPos);
            }}
            onUpdate={setCamData}
          />
        )}

        {/* 3D 에셋 렌더링 */}
        {osmData && origin && (
          <OsmFeatures
            // 5km 이동해서 원점이 바뀔 때만 통째로 새로 그립니다.
            key={`osm-layer-${origin[0]}`}
            data={osmData}
            options={{ showBuildings, showRoads: true }}
          />
        )}

        {/* 4. [해결] 무한 그리드 */}
        <Grid
          infiniteGrid
          fadeDistance={10000} // 멀어지면 안개처럼 사라짐
          sectionSize={mapConverter.metersToUnits(100)} // 100m 단위 큰 격자
          cellSize={mapConverter.metersToUnits(10)} // 10m 단위 작은 격자
          sectionColor="#444444"
          cellColor="#222222"
          position={[0, -0.02, 0]}
        />

        {/* 바닥 그리드 및 기준 사각형 */}
        {/* <gridHelper
          args={[gridStep * 100, 100, 0xffffff, 0x444444]}
          position={[0, -0.01, 0]}
        /> */}

        {/* 테스트용 빨간 사각형 (실제 100m x 100m) */}
        {/* <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0, 0]}>
          <planeGeometry
            args={[
              mapConverter.metersToUnits(100),
              mapConverter.metersToUnits(100),
            ]}
          />
          <meshBasicMaterial color="red" wireframe />
        </mesh> */}
      </Canvas>

      {/* UI 영역 */}
      <ScaleBar distance={camData.dist} />
      <Compass rotation={camData.rot} />
    </div>
  );
}

import { Compass } from './compass';
import { ScaleBar } from './scale-bar';
import { VoxelInspector } from './voxel-inspector';
import { RecenterButton } from './recenter-button';
import { LoadingSpinner } from '../toasts/loading-spinner';
import { LocationErrorToast } from '../toasts/location-error-toast';
import { useMapStore } from '../../model/map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useState } from 'react';
import { useDeviceOrientation } from '@/entities/device-location/model/use-orientation';

export function MapOverlays() {
  const status = useMapStore((state) => state.status);
  const deviceStatus = useDeviceLocationStore((state) => state.status);
  const isGlobalLoading = status === 'loading' || deviceStatus === 'loading';

  const deviceLocation = useDeviceLocationStore((state) => state.location);
  const message = useDeviceLocationStore((state) => state.message);

  const { requestPermission } = useDeviceOrientation();
  const [isPermissionGranted, setIsPermissionGranted] = useState(false);

  const handleEnableCompass = async () => {
    const granted = await requestPermission();
    setIsPermissionGranted(granted);
  };

  const { project, setOrigin, setCenter } = useMapStore();

  const handleRecenter = () => {
    if (deviceStatus === 'ready' && deviceLocation) {
      const newCenter = project(deviceLocation);
      setOrigin(deviceLocation);
      setCenter(newCenter);
    }
  };

  return (
    <>
      {/* 시선 방향 센서 권한 요청 UI */}
      {!isPermissionGranted && (
        <button
          onClick={handleEnableCompass}
          className="absolute bottom-28 left-1/2 -translate-x-1/2 z-50 px-5 py-2.5 bg-white/90 backdrop-blur-sm rounded-full shadow-xl border border-blue-100 flex items-center gap-2 transition-all active:scale-95"
        >
          <span style={{ fontSize: '18px' }}>📍</span>
          <span
            style={{ fontWeight: '600', color: '#007AFF', fontSize: '14px' }}
          >
            내 시선 방향 표시하기
          </span>
        </button>
      )}

      {/* 테스트용 버튼 */}
      <button
        onClick={() => setCenter({ x: 500, y: 0, z: 500 })}
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

      {/* 1. 정보성 위젯 (좌측 상단/하단) */}
      <div className="pointer-events-none absolute inset-0 z-50">
        <div className="pointer-events-auto absolute top-24 left-5">
          {/* 건물 표시 토글 등을 여기에 둘 수 있습니다 */}
        </div>

        <div className="pointer-events-auto absolute bottom-10 left-10 flex flex-col gap-4">
          <ScaleBar />
          <Compass />
        </div>

        <div className="pointer-events-auto absolute bottom-10 right-10">
          <VoxelInspector />
        </div>
      </div>

      {/* 2. 제어 버튼 */}
      <RecenterButton
        status={deviceStatus}
        deviceLocation={deviceLocation}
        onRecenter={handleRecenter}
      />

      {/* 3. 상태 알림 (로딩/에러) */}
      {isGlobalLoading && <LoadingSpinner />}
      {deviceStatus === 'error' && (
        <LocationErrorToast message={message || '위치 신호가 약합니다.'} />
      )}
    </>
  );
}

import { useMapStore } from '../../model/map-store';
import { useMemo } from 'react';

export function VoxelInspector() {
  // Store에서 필요한 상태와 액션 구독
  const center = useMapStore((state) => state.center);
  const viewState = useMapStore((state) => state.viewState);
  const hoveredPos = useMapStore((state) => state.hoveredPos);
  const unproject = useMapStore((state) => state.unproject);

  // center가 바뀔 때만 이 컴포넌트만 리렌더링됨
  const currentViewLocation = useMemo(
    () => (center ? unproject(center) : null),
    [center, unproject],
  );

  // 지도가 보고 있는 중심점 정보가 없으면 표시하지 않음
  if (!currentViewLocation || !center) return null;

  // 3D 공간의 유닛 좌표 (1유닛 = 1미터)
  const viewX = Math.floor(center.x);
  const viewY = Math.floor(center.y);
  const viewZ = Math.floor(center.z);

  // 도(Degree)를 방향 문자로 변환하는 간단한 헬퍼
  const getDirection = (deg: number) => {
    const normalized = ((deg % 360) + 360) % 360; // 0~360 유지
    if (normalized > 315 || normalized <= 45) return 'N';
    if (normalized > 45 && normalized <= 135) return 'E';
    if (normalized > 135 && normalized <= 225) return 'S';
    return 'W';
  };

  return (
    <div
      style={{
        position: 'absolute',
        right: '20px',
        top: '100px',
        padding: '12px',
        backgroundColor: 'rgba(0, 0, 0, 0.8)',
        color: '#00FF00', // 터미널 느낌의 녹색
        fontFamily: 'monospace',
        fontSize: '12px',
        borderRadius: '4px',
        zIndex: 1000,
        pointerEvents: 'none',
        boxShadow: '0 0 10px rgba(0,0,0,0.5)',
        border: '1px solid #333',
      }}
    >
      <div style={{ color: '#888', marginBottom: '4px' }}>
        [ WORLD COORDINATES ]
      </div>
      <div>LNG: {currentViewLocation.lng.toFixed(7)}</div>
      <div>LAT: {currentViewLocation.lat.toFixed(7)}</div>
      <div>ALT: {currentViewLocation.alt?.toFixed(1) ?? '0.0'}m</div>

      <div
        style={{ height: '1px', backgroundColor: '#444', margin: '10px 0' }}
      />

      <div style={{ color: '#888', marginBottom: '4px' }}>
        [ 3D ENGINE POS ]
      </div>
      <div style={{ color: '#FFFF00' }}>
        X: {viewX}, Y: {viewY}, Z: {viewZ}
      </div>

      <div
        style={{ height: '1px', backgroundColor: '#444', margin: '10px 0' }}
      />

      <div style={{ color: '#888', marginBottom: '4px' }}>[ 3D CAMERA ]</div>
      {/* [수정] Store의 altitude 반영 (휠 조작 시 실시간 변경) */}
      <div style={{ color: '#FF00FF', fontWeight: 'bold' }}>
        ALTITUDE: {Math.floor(viewState.altitude)}m
      </div>
      {/* [수정] Store의 bearing 및 pitch 반영 */}
      <div style={{ color: '#FFA500' }}>
        BEARING: {viewState.bearing.toFixed(1)}° (
        {getDirection(viewState.bearing)})
      </div>
      <div style={{ color: '#00AAFF' }}>
        PITCH: {viewState.pitch.toFixed(1)}°
      </div>
      <div style={{ color: '#888', fontSize: '10px' }}>
        DIST: {Math.floor(viewState.distance)}m
      </div>

      <div
        style={{ height: '1px', backgroundColor: '#444', margin: '10px 0' }}
      />

      <div style={{ color: '#888', marginBottom: '4px' }}>[ TARGET VOXEL ]</div>
      {hoveredPos ? (
        <div style={{ color: '#00FFFF' }}>
          CELL_X: {hoveredPos.x}
          <br />
          CELL_Y: {hoveredPos.y}
          <br />
          CELL_Z: {hoveredPos.z}
        </div>
      ) : (
        <div style={{ color: '#555' }}>OUT OF RANGE</div>
      )}
    </div>
  );
}

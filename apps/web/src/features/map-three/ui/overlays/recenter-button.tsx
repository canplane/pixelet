import type { Location } from '@/shared/types/map';
import type { DeviceLocationStatus } from '@/entities/device-location/model/device-location.store';

interface RecenterButtonProps {
  status: DeviceLocationStatus;
  deviceLocation: Location | null;
  onRecenter: () => void;
}

export function RecenterButton({
  status,
  deviceLocation,
  onRecenter,
}: RecenterButtonProps) {
  // 'ready' 상태이면서 위치 데이터가 존재할 때만 활성화
  const isEnabled = status === 'ready' && !!deviceLocation;
  const isLoading = status === 'loading';

  return (
    <button
      onClick={onRecenter}
      disabled={!isEnabled}
      style={{
        position: 'absolute',
        right: '20px',
        bottom: '30px',
        width: '52px',
        height: '52px',
        borderRadius: '50%',
        backgroundColor: isEnabled ? '#FFFFFF' : '#F8F8F8',
        border: '1px solid rgba(0,0,0,0.1)',
        boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
        cursor: isEnabled ? 'pointer' : 'not-allowed',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        transition: 'all 0.2s ease',
      }}
    >
      {isLoading ? (
        /* 버튼 내부용 미니 스피너 */
        <div
          style={{
            width: '22px',
            height: '22px',
            border: '2.5px solid rgba(0, 122, 255, 0.2)',
            borderTop: '2.5px solid #007AFF',
            borderRadius: '50%',
            animation: 'spin 0.8s linear infinite',
          }}
        />
      ) : (
        /* 위치 아이콘 (기존) */
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill={isEnabled ? '#007AFF' : '#A0A0A0'}
          style={{ transition: 'fill 0.2s' }}
        >
          <path d="M12 8c-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4-1.79-4-4-4zm8.94 3c-.46-4.17-3.77-7.48-7.94-7.94V1h-2v2.06C6.83 3.52 3.52 6.83 3.06 11H1v2h2.06c.46 4.17 3.77 7.48 7.94 7.94V23h2v-2.06c4.17-.46 7.48-3.77 7.94-7.94H23v-2h-2.06zM12 19c-3.87 0-7-3.13-7-7s3.13-7 7-7 7 3.13 7 7-3.13 7-7 7z" />
        </svg>
      )}

      {/* 애니메이션 정의 */}
      <style>{`
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
    </button>
  );
}

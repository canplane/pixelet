import { Locate, Navigation, Compass, Loader2 } from 'lucide-react';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { cn } from '@/shared/lib/utils';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { CircularButton } from '@/shared/ui/theme/glass/circular-button';
import { MotionContainer } from '@/shared/ui/layout/motion-container';
import { useMapFollowMode } from '@/features/map-three/model/use-map-follow-mode';
import { useSyncDeviceLocation } from '@/entities/device-location/model/use-sync-device-location';

export function LocationControl() {
  // 🎯 [Engine Start] 팔로우 모드 로직(카메라 이동 등)을 담당하는 엔진을 가동합니다.
  useMapFollowMode();

  const deviceStatus = useDeviceLocationStore((state) => state.status);
  const deviceLocation = useDeviceLocationStore((state) => state.location);
  const compassHeading = useDeviceLocationStore(
    (state) => state.compassHeading,
  );

  const { requestOrientation, isOrientationGranted } = useSyncDeviceLocation();

  const {
    canShowLocationControl,
    updateCenter,
    viewState,
    devicePos, // 🛰️ 현재 기기 위치
    isFollowMode,
    setIsFollowMode,
    triggerOrientationReset,
  } = useMapNavigation();

  const { bearing } = viewState;

  const isEnabled = deviceStatus === 'ready' && !!deviceLocation;
  const isLoading = deviceStatus === 'loading';

  const handleAction = async () => {
    if (!isEnabled || isLoading) return;

    // 1️⃣ 센터 이탈 상태라면? -> 내 위치로 즉시 복귀
    if (canShowLocationControl) {
      // 🛡️ [Refinement] Non-null assertion(!) 대신 조건문으로 안전하게 처리
      if (devicePos) {
        updateCenter(devicePos);
      }
      setIsFollowMode(false);
      return;
    }

    // 2️⃣ 센터에 있다면? -> 팔로우 모드(나침반 모드) 토글
    if (!isFollowMode) {
      const granted = isOrientationGranted || (await requestOrientation());
      if (granted) setIsFollowMode(true);
    } else {
      // 팔로우 모드 해제 시 북쪽 정렬
      setIsFollowMode(false);
      triggerOrientationReset();
    }
  };

  const getViewState = () => {
    if (isLoading)
      return { Icon: Loader2, className: 'animate-spin opacity-40' };

    if (canShowLocationControl)
      return { Icon: Locate, className: 'text-white/40 scale-90' };

    if (isFollowMode) {
      const rotation = compassHeading !== null ? compassHeading : 0;
      const finalRotation = rotation - bearing;

      return {
        Icon: Compass,
        style: { transform: `rotate(${finalRotation}deg)` },
        className:
          'text-blue-400 drop-shadow-[0_0_12px_rgba(96,165,250,0.8)] scale-110',
      };
    }

    return {
      Icon: Navigation,
      className:
        'text-white/90 drop-shadow-[0_0_10px_rgba(255,255,255,0.5)] group-hover:rotate-[45deg]',
    };
  };

  const { Icon, className, style } = getViewState();

  return (
    <MotionContainer variant="slideUp" delay={0.5}>
      <CircularButton
        onClick={handleAction}
        isActive={isFollowMode}
        disabled={!isEnabled || isLoading}
      >
        <Icon
          className={cn(
            'w-6 h-6 transition-all duration-500 ease-out',
            className,
          )}
          style={style}
        />
      </CircularButton>
    </MotionContainer>
  );
}

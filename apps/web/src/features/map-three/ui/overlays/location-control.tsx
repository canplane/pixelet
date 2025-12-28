import { Locate, Navigation, Compass, Loader2 } from 'lucide-react';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { cn } from '@/shared/lib/utils';
import { useDeviceOrientation } from '@/entities/device-location/model/use-orientation';
import { useMapNavigation } from '../../model/use-map-navigation';
import { CircularButton } from '@/shared/ui/theme/glass/circular-button';
import { MotionContainer } from '@/shared/ui/layout/motion-container';
import { useMapFollowMode } from '@/features/map-three/model/use-map-follow-mode';

export function LocationControl() {
  // 🎯 [Engine Start] 여기서 훅을 호출해야 Follow Mode 로직이 실제로 돌아갑니다.
  useMapFollowMode();

  const deviceStatus = useDeviceLocationStore((state) => state.status);
  const deviceLocation = useDeviceLocationStore((state) => state.location);

  // 1. 커스텀 훅 하나로 위치 관련 상태 모두 해결
  const {
    canShowLocationControl, // 🚀 훅에서 가져오기
    setCenter,
    viewState,
    devicePos,
    isFollowMode,
    setIsFollowMode,
    triggerOrientationReset,
  } = useMapNavigation();
  const { bearing } = viewState;

  // 🧭 기기의 방향(heading) 데이터 가져오기
  const { requestPermission, isPermissionGranted, compassHeading } =
    useDeviceOrientation();

  const isEnabled = deviceStatus === 'ready' && !!deviceLocation;
  const isLoading = deviceStatus === 'loading';

  const handleAction = async () => {
    if (!isEnabled || isLoading) return;

    // 1️⃣ 센터 이탈 상태라면? -> 내 위치로 즉시 복귀
    if (canShowLocationControl) {
      setCenter(devicePos!);
      setIsFollowMode(false);
      return;
    }

    // 2️⃣ 센터에 있다면? -> 팔로우 모드(나침반 모드) 토글
    if (!isFollowMode) {
      const granted = isPermissionGranted || (await requestPermission());
      if (granted) setIsFollowMode(true);
    } else {
      // 🚀 팔로우 모드 해제 시 북쪽 정렬도 함께 해주면 UX가 깔끔합니다.
      setIsFollowMode(false);
      triggerOrientationReset();
    }
  };

  const getViewState = () => {
    if (isLoading)
      return { Icon: Loader2, className: 'animate-spin opacity-40' };

    // 🚀 센터 이탈 시: "내 위치로" 아이콘
    if (canShowLocationControl)
      return {
        Icon: Locate,
        className: 'text-white/40 scale-90',
      };

    // 🚀 팔로우 모드: 나침반 회전 아이콘
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

    // 🚀 기본 상태: 길 찾기 화살표 아이콘
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
        {/* 🚀 아이콘에 style(transform) 적용 */}
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

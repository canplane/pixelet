import { Locate, Navigation, Compass, Loader2 } from 'lucide-react';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { cn } from '@/shared/lib/utils';
import { useDeviceOrientation } from '@/entities/device-location/model/use-orientation';
import { useMapNavigation } from '../../model/use-map-navigation';
import { CircularButton } from '@/shared/ui/theme/glass/circular-button';
import { MotionContainer } from '@/shared/ui/layout/motion-container';

export function LocationControl() {
  const deviceStatus = useDeviceLocationStore((state) => state.status);
  const deviceLocation = useDeviceLocationStore((state) => state.location);

  // 1. 커스텀 훅 하나로 위치 관련 상태 모두 해결
  const {
    setCenter,
    viewState,
    devicePos,
    isOffCenter,
    isFollowMode,
    setIsFollowMode,
  } = useMapNavigation();
  const { bearing } = viewState;

  // 🧭 기기의 방향(heading) 데이터 가져오기
  const { requestPermission, isPermissionGranted, compassHeading } =
    useDeviceOrientation();

  const isEnabled = deviceStatus === 'ready' && !!deviceLocation;
  const isLoading = deviceStatus === 'loading';

  const handleAction = async () => {
    if (!isEnabled || isLoading) return;

    if (devicePos && isOffCenter) {
      setCenter(devicePos);
      setIsFollowMode(false);
      return;
    }

    if (!isFollowMode) {
      const granted = isPermissionGranted || (await requestPermission());
      if (granted) setIsFollowMode(true);
    } else {
      setIsFollowMode(false);
    }
  };

  const getViewState = () => {
    if (isLoading)
      return { Icon: Loader2, className: 'animate-spin opacity-40' };

    if (isOffCenter)
      return {
        Icon: Locate,
        className: 'text-white/40 scale-90',
      };

    if (isFollowMode) {
      // 🚀 compassHeading이 null일 때는 회전하지 않도록 처리
      const rotation = compassHeading !== null ? compassHeading : 0;
      // 🧭 실제 북쪽(rotation)에서 지도가 돌아간 만큼(bearing) 보정
      const finalRotation = rotation - bearing;

      return {
        Icon: Compass,
        style: { transform: `rotate(${finalRotation}deg)` },
        className:
          'text-blue-400 drop-shadow-[0_0_12px_rgba(96,165,250,0.8)] scale-110 transition-transform duration-200 ease-out',
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

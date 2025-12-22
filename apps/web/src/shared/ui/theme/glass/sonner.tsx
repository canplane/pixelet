import { cn } from '@/shared/lib/utils';
import { Toaster as Sonner } from 'sonner';

type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      position="bottom-center" // 🚀 하단 중앙으로 이동
      className="toaster group"
      gap={0}
      // 🚀 컨테이너 자체를 화면 중앙에 꽉 차게 만듭니다.
      style={{
        left: '50%',
        transform: 'translateX(-50%)',
        width: '100%',
        display: 'flex',
        justifyContent: 'center',
        // 🚀 1. 컨테이너 전체는 마우스 이벤트를 무시 (지도 조작 가능)
        pointerEvents: 'none',
      }}
      toastOptions={{
        classNames: {
          toast: cn(
            // !static을 추가하여 Sonner의 절대 좌표 계산을 무시하고 Flex 중앙 정렬을 따르게 합니다.
            'group toast !bg-transparent !border-none !shadow-none !p-0 !flex !justify-center !w-full !transform-none !mb-10 !static',
            // 🚀 2. 실제 토스트 메시지 내용물만 다시 이벤트를 활성화 (클릭/닫기 가능)
            // 단, 현재 !bg-transparent로 감싸고 있는 형태라면 내부 '진짜' 콘텐츠 컴포넌트에
            // 'pointer-events-auto'를 적용해야 합니다.
            '[&>div]:pointer-events-auto',
          ),
        },
      }}
      {...props}
    />
  );
};

export { Toaster };

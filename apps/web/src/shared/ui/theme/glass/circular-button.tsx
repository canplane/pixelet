import * as React from 'react';
import { Button } from '@/shared/ui/theme/glass/button'; // 기존 Button 컴포넌트
import { cn } from '@/shared/lib/utils';

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  variant = 'glass', // 기본을 glass로 설정
  size = 'icon', // 지도 컨트롤은 보통 icon 사이즈
  disabled, // props에서 disabled 추출
  ...props
}: CircularButtonProps) {
  return (
    <Button
      variant={variant}
      size={size}
      disabled={disabled}
      className={cn(
        // [디자인 가이드라인 적용]
        // 1. 기본 크기와 곡률 (둥근 원형 유지)
        'relative w-14 h-14 rounded-full transition-all duration-500 active:scale-95 group',
        // 2. 테두리 및 하이라이트 효과 강화 (location-control에서 온 효과)
        'border-t-white/40 border-white/5 shadow-2xl',
        // 3. 활성화 상태일 때의 강조 (Follow Mode 등)
        isActive && 'ring-2 ring-blue-500/40 bg-blue-500/10',

        // 🚀 Disabled 상태 스타일 추가
        disabled &&
          'opacity-40 grayscale-[0.5] cursor-not-allowed pointer-events-none shadow-none border-t-white/10',

        className,
      )}
      {...props}
    >
      {/* 🚀 상단 글로우 하이라이트 (글로벌 디자인 효과 1) */}
      <div
        className={cn(
          'absolute top-0 left-1/2 -translate-x-1/2 w-10 h-5 bg-white/[0.08] rounded-full blur-[4px] pointer-events-none transition-opacity',
          disabled ? 'opacity-0' : 'opacity-100',
        )}
      />

      {/* 버튼 내부 아이콘 및 로직 */}
      <div
        className={cn(
          'flex items-center justify-center w-full h-full transition-colors',
          disabled ? 'text-white/20' : 'text-inherit',
        )}
      >
        {children}
      </div>

      {/* 🚀 하단 미세 그림자 (글로벌 디자인 효과 2) */}
      {!disabled && (
        <div className="absolute bottom-1 left-1/2 -translate-x-1/2 w-6 h-2 bg-white/5 rounded-full blur-[3px] pointer-events-none" />
      )}
    </Button>
  );
}

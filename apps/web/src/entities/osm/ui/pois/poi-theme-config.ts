/**
 * @file poi-theme-config.ts
 * POI 마커의 시각적 스타일(아이콘, 색상, 글로우 효과)을 정의하는 설정 파일입니다.
 * 컴포넌트에서 비즈니스 로직과 스타일 정의를 분리하여 가독성을 높입니다.
 */

export interface PoiStyle {
  icon: string;
  color: string;
  glow: string;
}

/**
 * 카테고리별 테마 설정 데이터
 * (컴포넌트 외부에 정적으로 정의하여 메모리 할당을 최적화합니다)
 */
const POI_THEME_MAP: Record<
  string,
  {
    icon: string;
    day: { color: string; glow: string };
    night: { color: string; glow: string };
  }
> = {
  cafe: {
    icon: '☕',
    day: {
      color: 'bg-[#FFD1B3] text-[#A35D36]',
      glow: 'shadow-[0_0_15px_rgba(163,93,54,0.4)]',
    },
    night: {
      color: 'bg-[#A35D36] text-[#FFD1B3]',
      glow: 'shadow-[0_0_15px_rgba(163,93,54,0.4)]',
    },
  },
  restaurant: {
    icon: '🍴',
    day: {
      color: 'bg-[#FFB7B2] text-[#B04A43]',
      glow: 'shadow-[0_0_15px_rgba(176,74,67,0.4)]',
    },
    night: {
      color: 'bg-[#B04A43] text-[#FFB7B2]',
      glow: 'shadow-[0_0_15px_rgba(176,74,67,0.4)]',
    },
  },
  shop: {
    icon: '🛍️',
    day: {
      color: 'bg-[#B2CEFE] text-[#4A6DAB]',
      glow: 'shadow-[0_0_15px_rgba(74,109,171,0.4)]',
    },
    night: {
      color: 'bg-[#4A6DAB] text-[#B2CEFE]',
      glow: 'shadow-[0_0_15px_rgba(74,109,171,0.4)]',
    },
  },
  tourism: {
    icon: '🏛️',
    day: {
      color: 'bg-[#E2CCFF] text-[#7A5DAC]',
      glow: 'shadow-[0_0_15px_rgba(122,93,172,0.4)]',
    },
    night: {
      color: 'bg-[#7A5DAC] text-[#E2CCFF]',
      glow: 'shadow-[0_0_15px_rgba(122,93,172,0.4)]',
    },
  },
  station: {
    icon: '🚉',
    day: {
      color: 'bg-[#FDFD96] text-[#8A8A3D]',
      glow: 'shadow-[0_0_15px_rgba(138,138,61,0.4)]',
    },
    night: {
      color: 'bg-[#8A8A3D] text-[#FDFD96]',
      glow: 'shadow-[0_0_15px_rgba(138,138,61,0.4)]',
    },
  },
};

/**
 * [Default Style] 매핑되지 않은 카테고리를 위한 기본값
 */
const DEFAULT_STYLE = {
  icon: '📍',
  day: { color: 'bg-white text-[#555]', glow: 'shadow-sm' },
  night: { color: 'bg-[#555] text-white', glow: 'shadow-sm' },
};

/**
 * [resolvePoiStyle]
 * 카테고리와 현재 테마(isNight)를 기반으로 최종 Tailwind 클래스셋을 반환합니다.
 */
export const resolvePoiStyle = (
  category: string,
  isNight: boolean,
): PoiStyle => {
  const config = POI_THEME_MAP[category] || DEFAULT_STYLE;
  const themeConfig = isNight ? config.night : config.day;

  return {
    icon: config.icon,
    color: themeConfig.color,
    glow: themeConfig.glow,
  };
};

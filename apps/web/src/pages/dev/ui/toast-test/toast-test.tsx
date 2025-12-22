import { LayoutGroup, AnimatePresence } from 'framer-motion';

import { useEffect, useState } from 'react';

import { DynamicIslandLayout } from '@/shared/ui/toasts/dynamic-island-layout';

export function TestDynamicIsland() {
  const INITIAL_DISTANCE = 2000; // 사용자님이 정해주신 기준값 (예: 2km)

  const [data, setData] = useState<{ user: any; dest: any } | null>(null);

  // 시뮬레이션: 3초 뒤에 위치 데이터가 잡힌다고 가정
  useEffect(() => {
    const timer = setTimeout(() => {
      setData({ user: { lat: 37.5 }, dest: { lat: 37.6 } });
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  return (
    <LayoutGroup>
      <div className="fixed top-0 left-0 right-0 flex flex-col items-center pt-4 z-[9999] pointer-events-none">
        <AnimatePresence mode="wait">
          {!data ? (
            /* Phase 1: 위치 찾는 중 (원형 중심) */
            <DynamicIslandLayout
              key="searching"
              title="GPS 신호 탐색 중"
              description="OFFLINE"
              icon={
                <div className="w-2 h-2 bg-amber-500 rounded-full animate-ping" />
              }
              className="border-amber-500/20"
            />
          ) : (
            /* Phase 2: 데이터 연결 성공 (INITIAL_DISTANCE 기준 확장) */
            <DynamicIslandLayout
              key="tracking"
              title="목적지 연결됨"
              description="LIVE"
              subDescription={`기준 거리 ${INITIAL_DISTANCE}m 내에 있습니다.`}
              statusColor="bg-green-500"
              icon={<div className="w-2 h-2 bg-green-500 rounded-full" />}
            />
          )}
        </AnimatePresence>

        {/* 테스트용 리셋 버튼 (개발 중에만 사용) */}
        <button
          onClick={() => setData(null)}
          className="mt-10 pointer-events-auto text-[10px] text-white/20 hover:text-white/50 transition-colors"
        >
          RESET STATE
        </button>
      </div>
    </LayoutGroup>
  );
}

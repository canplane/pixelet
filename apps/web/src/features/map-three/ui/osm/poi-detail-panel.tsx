import { motion, AnimatePresence } from 'framer-motion';
import { X, Navigation } from 'lucide-react';
import { useMapStore } from '../../model/map-store';
import { cn } from '@/shared/lib/utils';
import { Badge } from '@/shared/ui/theme/glass/badge';

export const PoiDetailPanel = () => {
  const setCenter = useMapStore((state) => state.setCenter);
  const selectedPoi = useMapStore((state) => state.selectedPoi);
  const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);

  if (!selectedPoi) return null;

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0, x: 20 }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: 20 }}
        className="w-64 h-fit pointer-events-auto font-sans"
      >
        {/* 곡률은 시스템 규칙(40px)을 유지하되 내부 여백을 줄임 */}
        <div className="liquid-glass w-full shadow-2xl rounded-[40px]">
          {/* 1. Header: p-5로 축소 */}
          <div className="p-5 pb-3 space-y-2.5">
            <div className="flex justify-between items-center">
              <Badge
                variant="glass"
                className="bg-white/10 text-white/50 border-white/10 text-[7px] tracking-[0.2em] px-1.5 h-3.5 rounded-full font-black"
              >
                {selectedPoi.category.toUpperCase()}
              </Badge>
              <button
                onClick={() => setSelectedPoi(null)}
                className="text-white/10 hover:text-white/40 transition-colors"
              >
                <X size={12} strokeWidth={3} />
              </button>
            </div>

            <h2 className="text-[18px] font-semibold text-white/90 tracking-tight break-all leading-[1.2]">
              {selectedPoi.name}
            </h2>
          </div>

          {/* 2. Content: 하단 패딩도 p-5로 맞추고 간격(gap) 최적화 */}
          <div className="px-5 pb-5 space-y-4">
            {/* Location Module: 패딩 p-3으로 압축 */}
            <div className="bg-white/[0.03] p-3 rounded-[20px] border border-white/5">
              <div className="flex justify-between items-center mb-2 px-0.5">
                <span className="text-[7px] font-black text-white/10 uppercase tracking-[0.2em]">
                  Spatial Data
                </span>
                <div className="w-1 h-1 rounded-full bg-blue-500/80 shadow-[0_0_6px_rgba(59,130,246,0.6)]" />
              </div>
              <div className="grid grid-cols-3 gap-0 border-t border-white/[0.02] pt-2">
                <MiniCoord label="X" value={selectedPoi.center.x.toFixed(0)} />
                <MiniCoord label="Y" value={selectedPoi.center.y.toFixed(0)} />
                <MiniCoord label="Z" value={selectedPoi.center.z.toFixed(0)} />
              </div>
            </div>

            {/* Tags: 뱃지 크기도 미세하게 조정 */}
            {Object.keys(selectedPoi.tags).length > 0 && (
              <div className="flex flex-wrap gap-1">
                {Object.entries(selectedPoi.tags).map(([key, value]) => (
                  <Badge
                    key={key}
                    variant="glass"
                    className="bg-white/[0.04] border-white/5 text-white/30 text-[8px] px-1.5 py-0 rounded-md font-bold"
                  >
                    {value}
                  </Badge>
                ))}
              </div>
            )}

            {/* Action Button: 높이를 14(56px)에서 11(44px) 정도로 낮춤 */}
            <button
              className={cn(
                'w-full flex items-center justify-center gap-2 h-11 rounded-full transition-all active:scale-[0.96]',
                'bg-white text-black hover:bg-neutral-100 shadow-lg text-[11px] font-bold',
              )}
              onClick={(e) =>
                setCenter({
                  x: selectedPoi.center.x,
                  y: selectedPoi.center.y,
                  z: selectedPoi.center.z,
                })
              }
            >
              <Navigation size={10} fill="currentColor" />
              Focus View
            </button>
          </div>
        </div>
      </motion.div>
    </AnimatePresence>
  );
};

const MiniCoord = ({ label, value }: { label: string; value: string }) => (
  <div className="flex flex-col items-center border-r last:border-none border-white/[0.03]">
    <span className="text-[6px] font-black text-white/5 uppercase mb-0.5">
      {label}
    </span>
    <span className="text-[11px] font-bold text-white/50 tabular-nums leading-none">
      {value}
    </span>
  </div>
);

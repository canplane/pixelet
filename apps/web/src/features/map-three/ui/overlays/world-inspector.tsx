import { useMapNavigation } from '@/entities/geo/model/use-map-navigation'; // 🚀 커스텀 훅 임포트
import { useMemo } from 'react';
import { motion } from 'framer-motion';
import { Compass, Box, Map as MapIcon, MousePointer2 } from 'lucide-react';
import * as THREE from 'three';
import { projection } from '@pixelet/core/shared/lib/map/projection';

import { Badge } from '@/shared/ui/theme/glass/badge';
import { Separator } from '@/shared/ui/theme/glass/separator';

const { unproject } = projection;

/**
 * [WorldInspector]
 * 엔진의 실시간 좌표계(WGS84, Coord)와 카메라 상태를 모니터링합니다.
 * XY 평면 지표면 지침과 Z축 고도 지침이 제대로 투영되고 있는지 확인하는 기준점이 됩니다.
 */
export function WorldInspector() {
  const { center, viewState, hoveredPos } = useMapNavigation();

  const currentViewLocation = useMemo(
    () => (center ? unproject(center) : null),
    [center, unproject],
  );

  if (!currentViewLocation || !center) return null;

  const viewVec = new THREE.Vector3(center.x, center.y, center.z);
  const pitchDeg = (viewState.polarAngle * 180) / Math.PI;

  const getDirection = (deg: number) => {
    const normalized = ((deg % 360) + 360) % 360;
    if (normalized > 315 || normalized <= 45) return 'N';
    if (normalized > 45 && normalized <= 135) return 'E';
    if (normalized > 135 && normalized <= 225) return 'S';
    return 'W';
  };

  return (
    <motion.div initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }}>
      {/* 🚀 CSS에 정의하신 .liquid-glass 클래스 적용 
          기존에 선언했던 중복된 유리 스타일을 이 클래스 하나로 대체합니다.
      */}
      <div className="liquid-glass w-64 p-5 font-sans select-none shadow-2xl">
        <div className="flex flex-col gap-6">
          {/* 1. World Location (toFixed 적용) */}
          <section className="space-y-3">
            <header className="text-[9px] font-medium flex items-center gap-1.5 uppercase tracking-[0.15em] text-white/40">
              <MapIcon size={10} /> World Coordinates
            </header>
            <div className="space-y-2 px-1">
              <div className="flex justify-between items-baseline">
                <span className="text-[10px] text-white/30 font-medium tracking-tight">
                  Latitude
                </span>
                <span className="text-sm font-light tracking-tight text-white/90 tabular-nums">
                  {currentViewLocation.lat.toFixed(6)}
                </span>
              </div>
              <div className="flex justify-between items-baseline">
                <span className="text-[10px] text-white/30 font-medium tracking-tight">
                  Longitude
                </span>
                <span className="text-sm font-light tracking-tight text-white/90 tabular-nums">
                  {currentViewLocation.lng.toFixed(6)}
                </span>
              </div>
            </div>
          </section>

          <Separator className="bg-white/5" />

          {/* 2. Grid & Camera Metrics */}
          <div className="grid grid-cols-2 gap-6 px-1">
            <div className="space-y-2.5">
              <header className="text-[8px] font-semibold text-white/20 tracking-[0.2em] uppercase flex items-center gap-1">
                <Box size={10} /> Engine
              </header>
              <div className="flex flex-col gap-1.5 text-[11px] font-light text-white/70 tabular-nums">
                <div className="flex justify-between">
                  <span className="text-white/20">X (E)</span>{' '}
                  {center.x.toFixed(1)}
                </div>
                <div className="flex justify-between">
                  <span className="text-white/20">Y (N)</span>{' '}
                  {center.y.toFixed(1)}
                </div>
                <div className="flex justify-between">
                  <span className="text-white/20">Z (A)</span>{' '}
                  {center.z.toFixed(1)}
                </div>
              </div>
            </div>

            <div className="space-y-2.5">
              <header className="text-[8px] font-semibold text-white/20 tracking-[0.2em] uppercase flex items-center gap-1">
                <Compass size={10} /> Camera
              </header>
              <div className="flex flex-col gap-1.5 tabular-nums">
                <div className="flex flex-col">
                  <span className="text-[11px] font-medium text-white/90 tracking-tight leading-tight">
                    ALT {viewState.altitude.toFixed(0)}m
                  </span>
                  <div className="flex justify-between items-center text-[10px] font-light text-white/40 tracking-tight">
                    <span>DST {viewState.distance.toFixed(0)}m</span>
                    <span>P {pitchDeg.toFixed(0)}°</span>
                  </div>
                </div>
                <div className="flex items-center gap-1.5 pt-0.5">
                  <Badge
                    variant="glass"
                    className="px-1.5 py-0 text-[8px] border-white/10 bg-white/5 text-white/50 font-medium rounded-[6px]"
                  >
                    {getDirection(viewState.bearing)}
                  </Badge>
                  <span className="text-[10px] text-white/30">
                    {viewState.bearing.toFixed(0)}°
                  </span>
                </div>
              </div>
            </div>
          </div>

          {/* 3. Target Voxel */}
          <section>
            {/* 🚀 곡률 보정: 부모 곡률(40px) - 패딩(20px) = 20px 
                임의 값 rounded-[20px]를 사용하여 시각적 평행을 맞춥니다.
            */}
            <div className="bg-white/[0.03] p-3.5 rounded-[20px] border border-white/5 transition-all hover:bg-white/[0.06]">
              <header className="text-[8px] font-semibold text-white/30 tracking-[0.2em] flex items-center gap-1.5 uppercase mb-2">
                <MousePointer2 size={10} /> Target Coordinates
              </header>
              {hoveredPos ? (
                <div className="flex justify-between text-[11px] font-light text-white/80 tracking-tight tabular-nums">
                  <span className="flex gap-1.5">
                    <span className="text-white/20">X</span>
                    {hoveredPos.x.toFixed(0)}
                  </span>
                  <span className="flex gap-1.5">
                    <span className="text-white/20">Y</span>
                    {hoveredPos.y.toFixed(0)}
                  </span>
                  <span className="flex gap-1.5">
                    <span className="text-white/20">Z</span>
                    {hoveredPos.z.toFixed(0)}
                  </span>
                </div>
              ) : (
                <div className="text-[10px] text-white/15 italic font-light tracking-wide py-0.5">
                  Awaiting target...
                </div>
              )}
            </div>
          </section>
        </div>
      </div>
    </motion.div>
  );
}

import { create } from 'zustand';
import { projection } from '@/shared/lib/geo/projection';
import type { LngLat, RelativeWorldPos } from '@/shared/types/map';
import { mapMath } from '@/shared/lib/geo/map-math';

interface MapState {
  origin: LngLat | null; // 상대 좌표의 기준점
  setOrigin: (lngLat: LngLat) => void;

  // 바로가기 함수
  project: (lngLat: LngLat) => RelativeWorldPos | null;
  unproject: (rel: RelativeWorldPos) => LngLat | null;
}

export const useMapStore = create<MapState>((set, get) => ({
  origin: null,
  setOrigin: (lngLat: LngLat) => set({ origin: lngLat }),

  // 바로가기 함수 (Facade)
  project: (lngLat: LngLat): RelativeWorldPos | null => {
    const { origin } = get();
    if (!origin) return null;

    const abs = mapMath.lngLatToAbs(lngLat);
    return mapMath.absToRel(abs, origin, projection.SCALE_FACTOR);
  },

  unproject: (relPos: RelativeWorldPos): LngLat | null => {
    const { origin } = get();
    if (!origin) return null;

    const abs = mapMath.relToAbs(relPos, origin, projection.SCALE_FACTOR);
    if (!abs) return null;
    return mapMath.absToLngLat(abs);
  },
}));

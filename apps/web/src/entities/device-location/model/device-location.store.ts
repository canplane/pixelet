import { create } from 'zustand';

interface DeviceLocationState {
  lngLat: [number, number] | null;
  accuracy: number | null;
  gpsHeading: number | null; // 이동 방향 (GPS)
  compassHeading: number | null; // 시선 방향 (센서)
  status: 'idle' | 'loading' | 'ready' | 'error';

  setGpsData: (data: {
    lngLat: [number, number];
    accuracy: number;
    heading: number | null;
  }) => void;
  setCompassHeading: (heading: number | null) => void;
}

export const useDeviceLocationStore = create<DeviceLocationState>((set) => ({
  lngLat: null,
  accuracy: null,
  gpsHeading: null,
  compassHeading: null,
  status: 'idle',

  setGpsData: (data) =>
    set({
      lngLat: data.lngLat,
      accuracy: data.accuracy,
      gpsHeading: data.heading,
      status: 'ready',
    }),
  setCompassHeading: (heading) => set({ compassHeading: heading }),
}));

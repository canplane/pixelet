import { useEffect, useState } from 'react';

type GeoState =
  | { status: 'idle' }
  | { status: 'loading' }
  | {
      status: 'ready';
      lngLat: [number, number];
      accuracy: number;
      heading: number | null;
      speed: number | null;
    }
  | { status: 'error'; message: string };

export function useGeolocation() {
  const [state, setState] = useState<GeoState>({ status: 'idle' });

  useEffect(() => {
    if (!('geolocation' in navigator)) {
      setState({ status: 'error', message: 'Geolocation not supported.' });
      return;
    }

    setState({ status: 'loading' });

    const id = navigator.geolocation.watchPosition(
      (pos) => {
        const { latitude, longitude, accuracy, heading, speed } = pos.coords;

        setState({
          status: 'ready',
          lngLat: [longitude, latitude],
          accuracy,
          heading, // 0~359도 (진북 기준), 정지 시 null
          speed, // m/s 단위, 정지 시 보통 0 혹은 null
        });
      },
      (err) => {
        setState({ status: 'error', message: err.message });
      },
      {
        enableHighAccuracy: true,
        maximumAge: 1000, // 더 신선한 데이터를 위해 1초로 단축
        timeout: 5000,
      },
    );

    return () => navigator.geolocation.clearWatch(id);
  }, []);

  return state;
}

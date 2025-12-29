import { useAppContext } from '@/app/providers/app-context';
import { useSyncDeviceLocation } from '@/entities/device-location/model/use-sync-device-location';

import { MapView } from '@/features/map-three/ui/viewport/map-view';
import { MapTiler } from '@/features/map-tiler/ui/map-tiler';

/** Map page */
function MapPage() {
  const { store } = useAppContext();

  // 🛰️ 여기서 호출하면 GPS와 나침반 데이터가 스토어에 쌓이기 시작합니다.
  useSyncDeviceLocation();

  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      {/* <main className="flex-1 container h-full" /> */}

      <MapView />
      {/* <MapTiler /> */}
    </div>
  );
}

export default MapPage;

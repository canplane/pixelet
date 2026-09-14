// @pxl/web/page/map_page.tsx

import { useGeolocation } from "../module/device/pose/use_geolocation.ts";
import { useDeviceOrientation } from "../module/device/pose/use_device_orientation.ts";

import { MapView } from "../module/map/map_view.tsx";

/** Map page */
function MapPage() {
  // start tracking device pose
  useGeolocation();
  useDeviceOrientation();

  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      {/* <main className="flex-1 container h-full" /> */}

      <MapView />
    </div>
  );
}

export default MapPage;

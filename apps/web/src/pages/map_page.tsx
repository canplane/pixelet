// @pxl/web/pages/map_page.tsx

import { useAppContext } from "@/app/providers/app_context.ts";

import { useGeolocation } from "../modules/device/pose/use_geolocation.ts";
import { useDeviceOrientation } from "../modules/device/pose/use_device_orientation.ts";

import { MapScene } from "../modules/map/scene.tsx";

/** Map page */
function MapPage() {
  const { store } = useAppContext();

  // start tracking device pose
  useGeolocation();
  useDeviceOrientation();

  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      {/* <main className="flex-1 container h-full" /> */}

      <MapScene />
    </div>
  );
}

export default MapPage;

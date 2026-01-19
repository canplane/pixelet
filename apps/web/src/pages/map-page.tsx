// @pxl/web/pages/map-page.tsx

import { useAppContext } from "@/app/providers/app-context.ts";

import { useGeolocation } from "../modules/device/pose/use-geolocation.ts";
import { useDeviceOrientation } from "../modules/device/pose/use-device-orientation.ts";

import { MapScene } from "../modules/map/scene.tsx";
import { MapTiler } from "@/modules/map-tiler/map-tiler.tsx";

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
      {/* <MapTiler /> */}
    </div>
  );
}

export default MapPage;

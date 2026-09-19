// @pxl/web/page/map-page.tsx

import { MapView } from "@/feature/map/ui/view.tsx";

/** Map page */
function MapPage() {
  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      <MapView />
    </div>
  );
}

export default MapPage;

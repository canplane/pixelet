// @pxl/web/app/page/map_page.tsx

import { MapView } from "../../ui/map/view.tsx";

/** Map page */
function MapPage() {
  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      <MapView />
    </div>
  );
}

export default MapPage;

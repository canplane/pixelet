import { useAppContext } from '@/app/providers/app-context';

import { MapScene } from '@/features/map-three/ui/scene/map-scene';
import { MapTiler } from '@/features/map-tiler/ui/map-tiler';

/** Index page */
function IndexPage() {
  const { store } = useAppContext();

  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      {/* <main className="flex-1 container h-full" /> */}

      <MapScene />
      {/* <MapTiler /> */}
    </div>
  );
}

export default IndexPage;

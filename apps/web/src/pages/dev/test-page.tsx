import { PoiList } from './ui/osm-data-test/poi-list';
import { ThreeTestCanvas } from './ui/three-test/three-test-canvas';

/** Test page */
function TestPage() {
  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      {/* Test OSM Data */}
      <PoiList />

      {/* Test Three.js */}
      {/* <ThreeTestCanvas /> */}
    </div>
  );
}

export default TestPage;

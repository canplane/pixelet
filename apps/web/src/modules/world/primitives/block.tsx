// @pxl/web/modules/world/primitives/block.tsx
// local block object primitive a parent local/world anchor

import { center3, type V3 } from "@pxl/spec/space/v3.ts";
import { to3 } from "@pxl/spec/space/adapter/three.ts";

type BlockProps = {
  base: V3;
  span: V3;

  color: string;
};
export const Block = (props: BlockProps) => {
  const { base, span, color } = props;

  return (
    <mesh name="block" position={to3(center3(base, span))} castShadow receiveShadow>
      <boxGeometry args={[span[0], span[2], span[1]]} />
      <meshStandardMaterial
        color={color}
        emissive={color}
        emissiveIntensity={0.06}
        roughness={1.0}
        metalness={0.0}
      />
    </mesh>
  );
};

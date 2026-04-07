// // @pxl/web/modules/world/primitives/block.tsx
// // local block object primitive a parent local/world anchor

// import type { f64 } from "@xpute/spec/lib/kit/word.ts";

// import { center3, fill3, type V3 } from "@pxl/spec/space/v3.ts";

// type BlockProps = {
//   base: V3;
//   span: V3 | f64;

//   color: string;
// };
// export const Block = (props: BlockProps) => {
//   const { base, span: _span, color } = props;
//   const span: V3 = typeof _span === "number" ? fill3(_span) : _span;

//   return (
//     <mesh name="block" position={center3(base, span)} castShadow receiveShadow>
//       <boxGeometry args={span} />
//       <meshStandardMaterial
//         color={color}
//         emissive={color}
//         emissiveIntensity={0.06}
//         roughness={1.0}
//         metalness={0.0}
//       />
//     </mesh>
//   );
// };

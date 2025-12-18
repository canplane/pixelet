import { useRef, useState, type JSX } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

function RotatingBox(props: JSX.IntrinsicElements['mesh']) {
  // Use TypeScript's Mesh type for the reference
  const meshRef = useRef<THREE.Mesh>(null!);

  // State for interactivity
  const [hovered, setHover] = useState(false);
  const [active, setActive] = useState(false);

  // Rotate the mesh every frame
  useFrame((state, delta) => {
    meshRef.current.rotation.x += delta;
    meshRef.current.rotation.y += delta * 0.5;
  });

  return (
    <mesh
      {...props}
      ref={meshRef}
      scale={active ? 1.5 : 1}
      onClick={() => setActive(!active)}
      onPointerOver={() => setHover(true)}
      onPointerOut={() => setHover(false)}
    >
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial color={hovered ? 'hotpink' : 'orange'} />
    </mesh>
  );
}

/** Test Three.js */
export function ThreeTestCanvas() {
  return (
    <div style={{ width: '100vw', height: '100vh', backgroundColor: '#111' }}>
      <Canvas camera={{ position: [0, 0, 5] }}>
        <ambientLight intensity={Math.PI / 2} />
        <spotLight
          position={[10, 10, 10]}
          angle={0.15}
          penumbra={1}
          decay={0}
          intensity={Math.PI}
        />
        <pointLight position={[-10, -10, -10]} decay={0} intensity={Math.PI} />

        <RotatingBox position={[-1.2, 0, 0]} />
        <RotatingBox position={[1.2, 0, 0]} />
      </Canvas>
    </div>
  );
}

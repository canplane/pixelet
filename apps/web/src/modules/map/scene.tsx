// @pxl/web/modules/map/scene.tsx

import { useRef } from "react";
import { Canvas } from "@react-three/fiber";
import * as THREE from "three";
import { Bloom, BrightnessContrast, EffectComposer, HueSaturation, N8AO, SMAA, TiltShift2, Vignette } from "@react-three/postprocessing";
import { BlendFunction } from "postprocessing";

import { useCamera } from "@/modules/camera/use_camera.ts";
import { CameraInput } from "@/modules/camera/input.tsx";
import { camera_cfg } from "@/modules/camera/config.ts";
import { post_cfg, render_cfg } from "@/modules/world/render/config.ts";
import { themes, type WorldTheme } from "@/modules/world/render/themes.ts";
import { useThemeKey } from "@/modules/world/render/theme_clock.ts";
import { FrameProvider } from "@/modules/world/frame/context.tsx";

import { MapEnvironment } from "./environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { useMapBootstrap } from "./navigation/use_map_bootstrap.ts";
import { useTileFetcher } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { useTerrainFollowSync } from "@/modules/world/terrain/use_terrain_follow_sync.ts";
import { MapOverlays } from "./ui/overlay/map_overlays.tsx";
import { MapHTMLPortal } from "./portal/html_portal.tsx";

import { MapToastController } from "./ui/toast/toast_controller.tsx";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

function CameraRig() {
  useCamera();
  return <CameraInput />;
}

export function MapScene() {
  useMapBootstrap();
  useTileFetcher();
  useTerrainFollowSync();

  const world_ref = useRef<THREE.Group>(null);

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <MapToastController />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        <Canvas
          dpr={1}
          gl={{
            antialias: false,
            powerPreference: "high-performance",
            alpha: false,
            stencil: false,
            depth: true,
          }}
          shadows
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          frameloop="always"
        >
          <FrameProvider>
            <CameraRig />

            {/* Lighting and sky environment */}
            <MapEnvironment />

            {/* World layers in the floating-origin local frame */}
            <World ref={world_ref} />
          </FrameProvider>

          {
            /*
            "Miniature diorama" post stack. Order matters:
            1. N8AO — real screen-space contact/crevice AO. Tried here early
               on and pulled back out with a (wrong) postmortem blaming a
               material-swap normal pre-pass. Re-read n8ao's actual source
               before retrying: it has no such pass — DepthDownSample.js
               reconstructs normals straight from the real depth buffer via
               screen-space derivatives, the same technique this terrain's
               own fragment shader already uses for its lighting, so the
               original garbage (a flat blue square, red scan-line noise)
               can't have been a displacement/normal mismatch. Retried once
               the shadow pipeline (lights:true, shared uniforms) and
               multisampling were both stable and it renders correctly.
               aoRadius/intensity (post_cfg.AO) needed real tuning once it
               actually worked, not just non-garbage defaults: at the
               original radius, every riser of this terrain's LOD-stepped
               geometry shadowed several neighbors up the same slope,
               reading as repeated contour-line banding on any terraced
               area rather than soft contact shadow — confirmed the shader's
               own AO/height-tint effects weren't the cause by bypassing
               them and watching the exact same bands survive unchanged.
               Shrunk to roughly one cell-neighborhood's scale instead.
            2. Bloom — glow on the few genuinely bright things (sun disc, night
               lights); the high luminance threshold keeps daytime scenes from
               washing out.
            3. Color grade (contrast/saturation) — the flat/raw output reads
               washed out next to the punchy reference stills; this is cheap
               and global.
            4. TiltShift2 — screen-space blur band, the actual "macro lens"
               illusion that sells the miniature look. Works because this
               camera holds a fixed top-down-ish angle, so screen-Y already
               correlates with depth the way a real tilt-shift lens exploits.
            5. Vignette — subtle framing, closes out the grade.
            6. SMAA last — edge cleanup after every other pass has run.
          */
          }
          {
            /*
            multisampling was 0 (SMAA-only) — fine for clean geometric edges,
            but this terrain is thousands of small flat-shaded boxes, and at
            any real distance many of them minify into one pixel. That's not
            an edge SMAA detects; it's the same class of problem as the grid/
            grain shader fades above, just from geometry+shading instead of a
            procedural pattern, and only real supersampling fixes it. Real
            MSAA rather than a higher value here, since dpr is pinned to 1
            (see the Canvas props) specifically to keep the close-zoom cost
            (still-open lag investigation) from getting worse — worth
            revisiting together if this turns out too expensive.
          */
          }
          <EffectComposer multisampling={4}>
            <N8AO
              quality={post_cfg.AO.QUALITY}
              halfRes={post_cfg.AO.HALF_RES}
              aoRadius={post_cfg.AO.RADIUS}
              intensity={post_cfg.AO.INTENSITY}
              distanceFalloff={post_cfg.AO.DISTANCE_FALLOFF}
              color={post_cfg.AO.COLOR}
            />
            <Bloom
              intensity={post_cfg.BLOOM.INTENSITY}
              luminanceThreshold={post_cfg.BLOOM.LUMINANCE_THRESHOLD}
              luminanceSmoothing={post_cfg.BLOOM.LUMINANCE_SMOOTHING}
              mipmapBlur={post_cfg.BLOOM.MIPMAP_BLUR}
            />
            <BrightnessContrast brightness={post_cfg.GRADE.BRIGHTNESS} contrast={post_cfg.GRADE.CONTRAST} />
            <HueSaturation saturation={post_cfg.GRADE.SATURATION} />
            <TiltShift2
              blur={post_cfg.TILT_SHIFT.BLUR}
              taper={post_cfg.TILT_SHIFT.TAPER}
              start={post_cfg.TILT_SHIFT.START}
              end={post_cfg.TILT_SHIFT.END}
              direction={post_cfg.TILT_SHIFT.DIRECTION}
              samples={post_cfg.TILT_SHIFT.SAMPLES}
            />
            <Vignette
              blendFunction={BlendFunction.NORMAL}
              darkness={post_cfg.VIGNETTE.DARKNESS}
              offset={post_cfg.VIGNETTE.OFFSET}
            />
            <SMAA />
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <MapOverlays />
    </div>
  );
}

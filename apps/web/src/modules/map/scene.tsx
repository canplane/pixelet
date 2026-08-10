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
          // dpr={[1, 2]}: real spatial supersampling, not just edge AA.
          // MSAA (tried below, previously multisampling={4}) only resolves
          // per-sample *coverage/depth* — the fragment shader itself still
          // runs once per output pixel, so it never re-evaluates shading at
          // sub-pixel positions. That's fine for a triangle's silhouette
          // edge, but this terrain is thousands of independently
          // flat-shaded boxes that minify well below one pixel at any real
          // distance (see the EffectComposer comment below) — the visible
          // artifact there is shading-value flicker between neighboring
          // facets sharing a pixel, which MSAA structurally cannot fix.
          // Rendering the whole canvas (WebGL draw + the full post stack)
          // at up to 2x linear resolution and letting the browser's own
          // canvas-to-CSS downscale do a real per-pixel color average fixes
          // both classes of aliasing at once. Was pinned to a flat 1
          // specifically to bound close-zoom cost; revisiting now that
          // CELL_SHAMT going -2 -> -1 cut instanced-terrain vertex/fragment
          // load ~4x, freeing up roughly the budget this costs (dpr 2
          // means ~4x the pixels, so ~4x the fragment-shading work of the
          // whole pipeline, post stack included) — if this turns out too
          // expensive on real devices, that's the number to re-check
          // against, and the [1,2] cap keeps it from ever exceeding 2x even
          // on a 3x/4x-DPR phone.
          dpr={[1, 2]}
          gl={{
            antialias: false,
            powerPreference: "high-performance",
            alpha: false,
            stencil: false,
            depth: true,
          }}
          // "percentage" (THREE.PCFShadowMap) instead of bare `shadows`
          // (which defaults to PCFSoft) — the installed three.js version
          // deprecated PCFSoftShadowMap and silently substitutes
          // PCFShadowMap on every single render call, spamming devtools
          // with the same warning every frame. Requesting PCFShadowMap
          // directly is a no-op behavior-wise (matches what three.js was
          // already falling back to) but skips the deprecated codepath
          // entirely.
          shadows="percentage"
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          // "always" rendered every frame unconditionally, even fully at
          // rest. "demand" only renders when something calls invalidate() —
          // every useFrame that changes something over time now has to ask
          // for its own next frame explicitly: use_camera.ts (camera
          // damping), use_frame_translation.ts/use_frame_rotation.ts
          // (floating-origin frame damping), environment.tsx (real-time
          // sun/moon + theme color lerps, via a plain interval since
          // there's no prop/store change to hang a wake-up effect off of),
          // and location_puck.tsx's beacon spin/bob (which never converges
          // by design, so it just keeps re-invalidating for as long as it's
          // visible — see its own comment for why that puts a floor under
          // how idle this can ever get). chunk.tsx's LOD/shadow-eligibility
          // recheck needed no changes: it's purely a function of camera-to-
          // chunk distance, so it can only ever have something new to do on
          // a frame the camera damping above already caused to render.
          frameloop="demand"
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
            multisampling left at its default (0, off): the Canvas's dpr
            prop above now does real supersampling (the whole canvas,
            including this entire post stack, renders at up to 2x linear
            resolution and gets box-downsampled by the browser), which
            covers what MSAA was here for and then some — MSAA only
            resolves per-sample coverage/depth for triangle edges, never
            re-running the fragment shader per sample, so it could not touch
            the shading-domain flicker this terrain's thousands of small
            flat-shaded boxes produce once they minify below a pixel.
            Stacking MSAA on top of the dpr supersampling would add further
            coverage-sample cost without fixing an aliasing class dpr
            doesn't already cover, so it's dropped rather than kept
            alongside it.
          */
          }
          <EffectComposer>
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

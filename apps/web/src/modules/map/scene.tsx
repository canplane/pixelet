// @pxl/web/modules/map/scene.tsx

import { useRef } from "react";
import { Canvas } from "@react-three/fiber";
import * as THREE from "three";
import { BrightnessContrast, EffectComposer, HueSaturation, Vignette } from "@react-three/postprocessing";
import { BlendFunction } from "postprocessing";

import { useCamera } from "@/modules/camera/use_camera.ts";
import { CameraInput } from "@/modules/camera/input.tsx";
import { camera_cfg } from "@/modules/camera/config.ts";
import { post_cfg, render_cfg } from "@/modules/world/environment/config.ts";
import { themes, type WorldTheme } from "@/modules/world/environment/themes.ts";
import { useThemeKey } from "@/modules/world/environment/theme_clock.ts";
import { FrameProvider } from "@/modules/world/frame/context.tsx";

import { MapEnvironment } from "./environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { useMapBootstrap } from "./navigation/use_map_bootstrap.ts";
import { useTileFetcher } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { useTerrainFollowSync } from "@/modules/world/terrain/use_terrain_follow_sync.ts";
import { MapOverlays } from "./ui/overlay/map_overlays.tsx";
import { MapHTMLPortal } from "./portal/html_portal.tsx";
import { DevPanel } from "@/app/dev/dev_panel.tsx";

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
          // TEMPORARY 2026-08-13: pinned to [1,1] to isolate resolution's
          // own contribution to the desktop/Retina perf investigation
          // (dpr=2 on a Retina display means 4x the pixels, hence ~4x the
          // fragment-shading work, vs dpr=1) — separate from chunk-count/
          // frustum-culling effects, which don't depend on pixel count at
          // all. Kept at [1,1] deliberately — do not raise back to [1,2],
          // that quadruples fragment-shading cost.
          dpr={[1, 1]}
          gl={{
            antialias: false,
            powerPreference: "high-performance",
            alpha: false,
            stencil: false,
            depth: true,
          }}
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          // Confirmed live via profiling: getProgramInfoLog/getShaderInfoLog
          // (three.js's post-compile GL log validation, run by default on
          // every new WebGLProgram — renderer.debug.checkShaderErrors
          // defaults to true) were the dominant main-thread cost while
          // panning, ~90% of a recorded frame's self time. These calls force
          // a CPU-GPU sync point to read back the compile/link log — real
          // cost even though our own shader source is a stable module-level
          // constant (mat.ts) that three.js's program cache already
          // reuses across every chunk; the validation step still runs once
          // per *material instance* (now one per Chunk/LOD/material
          // bucket, previously one per L2) regardless of whether the
          // underlying compiled program is shared. Standard production
          // three.js/r3f advice (WebGLRenderer.debug.checkShaderErrors's own
          // doc comment) is to disable this once shaders are known-good.
          onCreated={(state) => {
            state.gl.debug.checkShaderErrors = false;
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
          // how idle this can ever get). chunk_stage.tsx additionally
          // invalidates while its global reveal/LOD-remesh budgets still
          // have queued work, then becomes camera-driven again once those
          // queues drain.
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
            "Miniature diorama" post stack — N8AO/Bloom/TiltShift2/SMAA
            (2026-08-13) disabled, not deleted: confirmed real cost via live
            profiling of the terrain pipeline (WebGLRenderer.renderBufferDirect/
            setProgram time scaling with the post stack), and none of the four
            are things a Minecraft-default-options level would even offer as a
            toggle (SSAO/bloom/tilt-shift/post-AA are shader-pack territory,
            not vanilla settings) — so turned fully off rather than kept behind
            a runtime option. Commented out (not removed) so the "miniature
            diorama" look this was built for is easy to bring back later, not
            lost. Order mattered when these were active:
            1. N8AO — real screen-space contact/crevice AO. See git history
               (this comment block, pre-2026-08-13) for the normal-pass/
               radius-tuning notes from when it was built.
            2. Bloom — glow on the few genuinely bright things (sun disc, night
               lights); the high luminance threshold keeps daytime scenes from
               washing out.
            3. Color grade (contrast/saturation) — the flat/raw output reads
               washed out next to the punchy reference stills; this is cheap
               and global, stays active below.
            4. TiltShift2 — screen-space blur band, the actual "macro lens"
               illusion that sold the miniature look. Worked because this
               camera holds a fixed top-down-ish angle, so screen-Y already
               correlates with depth the way a real tilt-shift lens exploits.
            5. Vignette — subtle framing; cheap, stays active below.
            6. SMAA last — edge cleanup after every other pass ran.
          */
          }
          {
            /*
            multisampling left at its default (0, off): the Canvas's dpr
            prop above already does real supersampling (the whole canvas
            renders at up to 2x linear resolution and gets box-downsampled
            by the browser) — MSAA only resolves per-sample coverage/depth
            for triangle edges, never re-running the fragment shader per
            sample, so it could not touch the shading-domain flicker this
            terrain's thousands of small flat-shaded boxes produce once
            they minify below a pixel. Independent of SMAA (above) being on
            or off — this is about the Canvas's own `multisampling` prop.
          */
          }
          <EffectComposer>
            {
              /* <N8AO
              quality={post_cfg.AO.QUALITY}
              halfRes={post_cfg.AO.HALF_RES}
              aoRadius={post_cfg.AO.RADIUS}
              intensity={post_cfg.AO.INTENSITY}
              distanceFalloff={post_cfg.AO.DISTANCE_FALLOFF}
              color={post_cfg.AO.COLOR}
            /> */
            }
            {
              /* <Bloom
              intensity={post_cfg.BLOOM.INTENSITY}
              luminanceThreshold={post_cfg.BLOOM.LUMINANCE_THRESHOLD}
              luminanceSmoothing={post_cfg.BLOOM.LUMINANCE_SMOOTHING}
              mipmapBlur={post_cfg.BLOOM.MIPMAP_BLUR}
            /> */
            }
            <BrightnessContrast brightness={post_cfg.GRADE.BRIGHTNESS} contrast={post_cfg.GRADE.CONTRAST} />
            <HueSaturation saturation={post_cfg.GRADE.SATURATION} />
            {
              /* <TiltShift2
              blur={post_cfg.TILT_SHIFT.BLUR}
              taper={post_cfg.TILT_SHIFT.TAPER}
              start={post_cfg.TILT_SHIFT.START}
              end={post_cfg.TILT_SHIFT.END}
              direction={post_cfg.TILT_SHIFT.DIRECTION}
              samples={post_cfg.TILT_SHIFT.SAMPLES}
            /> */
            }
            <Vignette
              blendFunction={BlendFunction.NORMAL}
              darkness={post_cfg.VIGNETTE.DARKNESS}
              offset={post_cfg.VIGNETTE.OFFSET}
            />
            {/* <SMAA /> */}
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <MapOverlays />

      {
        /* Debugging UI — moved here from app.tsx (was a root-level sibling
        of AppRouter) so its z-10 actually compares against ModeBar's z-40
        (bottom_overlays.tsx). This div's own `fixed inset-0` establishes
        its own stacking context per spec (position:fixed always does,
        independent of z-index), so a z-index living OUTSIDE it — like
        DevPanel's old root-level placement — never actually competed
        against anything inside; it just painted above the whole subtree
        regardless of the number. Living inside now, the two z-indexes
        finally resolve against each other as their numbers suggest. */
      }
      {__IS_DEV__ && <DevPanel />}
    </div>
  );
}

// @pxl/web/module/map/scene.tsx

import { type JSX, useEffect, useMemo, useRef } from "react";
import { Canvas, useThree } from "@react-three/fiber";
import type { Pass } from "postprocessing";
import * as THREE from "three";
import { Bloom, BrightnessContrast, EffectComposer, GodRays, HueSaturation, N8AO, SMAA, Vignette } from "@react-three/postprocessing";

import { SCENE_DEPTH, SceneDepthPass } from "@/render/scene_depth.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { BlendFunction, SMAAPreset } from "postprocessing";

import { useCamera } from "@/module/camera/use_camera.ts";
import { CameraInput } from "@/module/camera/input.tsx";
import { camera_cfg } from "@/module/camera/config.ts";
import { post_cfg, render_cfg } from "@/module/world/environment/config.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { themes, type WorldTheme } from "@/module/world/environment/theme.ts";
import { useThemeKey } from "@/module/world/environment/theme_clock.ts";
import { FrameProvider } from "@/module/world/frame/context.tsx";

import { MapEnvironment } from "./environment.tsx";
import { TickDriver } from "@/module/world/sched/tick.ts";
import { World } from "@/module/world/world.tsx";
import { useMapBootstrap } from "./navigation/use_map_bootstrap.ts";
import { useTileFetcher } from "@/module/world/terrain/use_tile_fetcher.ts";
import { usePoiFetcher } from "@/module/world/poi/use_poi_fetcher.ts";
import { useGroundSnapSync } from "@/module/world/terrain/use_ground_snap_sync.ts";
import { Overlays } from "./ui/overlay/overlays.tsx";
import { MapHTMLPortal } from "./portal/html_portal.tsx";

import { GpuCanvas } from "@/render/gpu/stage.tsx";
import { ToastController } from "./ui/toast/toast_controller.tsx";
import { LedgerOverlay } from "@/module/world/sched/ledger_overlay.tsx";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

function CameraRig() {
  useCamera();
  return <CameraInput />;
}

/**
 * frameloop="demand" only repaints on invalidate() — a plain prop change
 * reaching Canvas/EffectComposer from outside the Canvas tree (MapScene's
 * own useSettingsStore reads, e.g. dpr or an effect toggle) does update the
 * underlying render/postprocessing state, but never triggers a new frame on
 * its own, so the change stays invisible until something else happens to
 * invalidate. Mounted inside the Canvas so useThree works, purely to force
 * that missing invalidate whenever any dev render-toggle changes.
 */
function DevRenderToggleInvalidator(props: { deps: readonly unknown[] }) {
  const invalidate = useThree((state) => state.invalidate);
  useEffect(() => {
    invalidate();
  }, [invalidate, ...props.deps]);
  return null;
}

/**
 * Hands the R3F camera to passes that read `mainCamera`. The composer's
 * setMainCamera is what would, and @react-three/postprocessing never calls
 * it, so SceneDepthPass never learned its camera and never marked its depth
 * ready — billboards behind terrain were never occluded.
 */
function MainCameraLink({ passes }: { passes: readonly Pass[] }) {
  const camera = useThree((state) => state.camera);
  useEffect(() => {
    for (const pass of passes) pass.mainCamera = camera;
  }, [camera, passes]);
  return null;
}

/**
 * The composer's multisample count. Four is the conventional count for a
 * forward renderer and what the desktop was tuned at
 * (issue/20260911-taa-tried-and-closed.md). A phone gets two: at its
 * native dpr the difference is not visible, and the multisampled buffer is
 * the largest single allocation the page makes — colour and depth per
 * sample over three megapixels, a hundred and fifty megabytes at four
 * samples and half-float, on a device whose tab reloads past about a
 * gigabyte and a half.
 */
const MSAA_SAMPLES = HANDHELD ? 2 : 4;

/**
 * Edge anti-aliasing for displays that do not get it from resolution.
 *
 * At dpr 2 the canvas has four samples per CSS pixel and the browser's
 * downscale averages them; edges are clean and a post AA pass would find
 * nothing to fix, so it is not paid for. At dpr 1 — a desktop monitor, a
 * cheap laptop — there is one sample per pixel and MSAA (the composer's
 * multisampling) only resolves coverage, not the shading of the flat-shaded
 * boxes that minify below a pixel; SMAA's edge search picks up what MSAA
 * leaves, at the cost of one fullscreen pass on a display that has the
 * pixel budget to spare. Phones are dpr 3 capped to 2 and take the first
 * path; this is for the desktop that would otherwise draw the jaggiest
 * picture of all. Read from r3f's own viewport.dpr, the ratio the canvas
 * was actually given, rather than window.devicePixelRatio and a second
 * clamp. Not user-facing: the choice follows the display, as the dpr does.
 * Untested on a dpr-1 device as of 2026-09-13; revisit with one in hand.
 */
function AutoSmaa() {
  const dpr = useThree((state) => state.viewport.dpr);
  if (dpr >= post_cfg.SMAA_BELOW_DPR) return null;
  // MEDIUM is the library default; HIGH searches longer edges, a moderate
  // step up rather than ULTRA.
  return <SMAA preset={SMAAPreset.HIGH} />;
}

export function MapScene() {
  useMapBootstrap();
  useTileFetcher();
  usePoiFetcher();
  useGroundSnapSync();

  // One instance for the app's life. Passes are stateful (render target,
  // materials) and the composer adds/removes them by identity, so rebuilding
  // one per render would leak targets and thrash the chain.
  const scene_depth_pass = useMemo(() => new SceneDepthPass(), []);

  const world_ref = useRef<THREE.Group>(null);
  // Shared with environment.tsx (MapEnvironment writes this mesh's position
  // every frame off the real-time sun direction) — see SUN_PROXY_DISTANCE's
  // own doc comment there for why GodRays needs a real occludable Mesh
  // rather than the sky dome's shader-drawn sun disc.
  const sun_proxy_ref = useRef<THREE.Mesh>(null);

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];

  const show_ao = useSettingsStore((state) => state.show_ao);
  const show_bloom = useSettingsStore((state) => state.show_bloom);
  const show_god_rays = useSettingsStore((state) => state.show_god_rays);
  const show_shadow_map = useSettingsStore((state) => state.show_shadow_map);
  // Dev bisect of the pixel pipeline (schema.ts's render_path). `no_post`
  // leaves the composer out entirely, so r3f draws the scene straight to
  // the canvas; the scene depth pass goes with it, and MainCameraLink has
  // nothing to link.
  const render_path = useSettingsStore((state) => state.render_path);
  const with_post = render_path !== "no_post";
  // Half-float only when a pass needs values past one. The composer's
  // default is half-float always, which doubles every target it owns — the
  // multisampled buffer above all — and nothing in the shipped stack
  // writes outside [0, 1].
  const frame_buffer_type = show_bloom || show_god_rays ? THREE.HalfFloatType : THREE.UnsignedByteType;
  // Leaving the composer removes the depth pass without disposing it, so
  // its last copy would stay marked ready and badges would occlude against
  // a frame that is no longer being drawn. The pass marks it ready again
  // on its next render.
  useEffect(() => {
    if (!with_post) SCENE_DEPTH.ready.value = 0;
  }, [with_post]);

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <ToastController />
      <LedgerOverlay />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        <Canvas
          // The device's own pixel ratio, capped at 2. This is not a cost
          // the user is asked about, because it is not an extra cost: [1, 2]
          // is r3f's clamp(window.devicePixelRatio, 1, 2), so a dpr-1 laptop
          // draws at 1x and a Retina display at 2x — the resolution every
          // other layer of the page is already drawn at on that device, and
          // the pixel count the device was built to fill. Rendering the
          // canvas at 1x on a 2x display would be the odd one out, blurred
          // by the browser's upscale, for a saving the rest of the page does
          // not make. The cap is where the real saving is: a 3x/4x phone
          // would otherwise pay nine or sixteen times the fragment work of
          // 1x for a difference the eye does not see at that density, and
          // the mobile GPU is the one that cannot afford it — everything
          // here has to run on a phone, and 2x is what a phone gets.
          // What native resolution buys over MSAA (the composer's
          // multisampling below) is shading at sub-pixel positions: MSAA
          // resolves coverage and depth per sample but shades once per
          // pixel, so the thousands of flat-shaded boxes that minify below a
          // pixel still flicker between neighbouring facets; more pixels is
          // the only thing that averages that away. Where there are no more
          // pixels — dpr 1 — AutoSmaa below adds an edge pass instead. A
          // user-facing choice between these was offered until 2026-09-13
          // and removed: the right answer is the device's, not the user's.
          dpr={post_cfg.DPR}
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
          // "percentage" (THREE.PCFShadowMap), not the bare `shadows` prop
          // (defaults to the deprecated PCFSoftShadowMap, which the
          // installed three.js version silently substitutes anyway while
          // spamming a deprecation warning every frame — see git history on
          // this exact string). false when show_shadow_map is off: r3f skips
          // enabling renderer.shadowMap entirely rather than paying for a
          // shadow-capable renderer state no light ever casts into.
          shadows={show_shadow_map ? "percentage" : false}
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
          // doc comment) is to disable this once shaders are known-good —
          // but "known-good" is exactly what onBeforeCompile's hand-written
          // GLSL string surgery (chunk_stage.tsx) can't assume: a broken
          // .replace() target or an undeclared identifier fails silently
          // with this off, no console error at all (found the hard way
          // debugging rim light). Dev keeps checks on to actually catch
          // that; only production pays zero for the confirmed cost above.
          onCreated={(state) => {
            state.gl.debug.checkShaderErrors = import.meta.env.DEV;
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
          <DevRenderToggleInvalidator deps={[show_ao, show_bloom, show_god_rays, show_shadow_map, render_path]} />
          {
            /* The one useFrame in this Canvas. Every per-frame subscriber in
              the world registers a step into a phase of this tick instead
              (sched/tick.ts) — one order, one budget. */
          }
          <TickDriver />
          <FrameProvider>
            <CameraRig />

            {/* Lighting and sky environment */}
            <MapEnvironment sun_proxy_ref={sun_proxy_ref} />

            {/* World layers in the floating-origin local frame */}
            <World ref={world_ref} />
          </FrameProvider>
          {with_post && <MainCameraLink passes={[scene_depth_pass]} />}

          {
            /*
            The post stack. N8AO, Bloom and GodRays default OFF: each is a
            real pass with a confirmed cost from live profiling of the
            terrain pipeline (WebGLRenderer.renderBufferDirect/setProgram
            time scaling with the stack), and none is something a vanilla
            options screen would offer. They are settings (app/setting/
            schema.ts's show_ao, and the dev-only show_bloom/show_god_rays)
            rather than on by default. The "miniature diorama" passes that
            used to sit here — TiltShift2 and ChromaticAberration — were
            removed 2026-09-13: neither had earned a place, and a setting
            nobody turns on is a pass nobody maintains. Order when on:
            1. N8AO — real screen-space contact/crevice AO. See git history
               (this comment block, pre-2026-08-13) for the normal-pass/
               radius-tuning notes from when it was built.
            2. Bloom — glow on the few genuinely bright things (sun disc, night
               lights); the high luminance threshold keeps daytime scenes from
               washing out.
            3. Color grade (contrast/saturation) — the flat/raw output reads
               washed out next to the punchy reference stills; this is cheap
               and global, stays active unconditionally below.
            4. Vignette — subtle framing; cheap, stays active unconditionally
               below.
            5. SMAA, on dpr-1 displays only — AutoSmaa above.
          */
          }
          {
            /*
            multisampling={4}: 4x MSAA on the composer's input buffer, over
            the native dpr above. MSAA is the conventional
            anti-aliasing for a forward renderer, and the distant boxes that
            shrink below a pixel are a coverage problem, which is what it
            resolves. It had been running at 8 — @react-three/postprocessing
            3's default, while this comment said 0 — which put GPU p95 at
            17 ms against 8 ms with none. 4 is the common count; without any,
            the far terrain shimmered. A camera-reprojection TAA was tried in
            its place and closed: issue/20260911-taa-tried-and-closed.md.
          */
          }
          {with_post && (
            <EffectComposer multisampling={render_path === "no_msaa" ? 0 : MSAA_SAMPLES} frameBufferType={frame_buffer_type}>
              {
                // EffectComposerProps types children as JSX.Element | JSX.Element[],
                // not accepting `cond && <X/>`'s `false` branch — build the array
                // and filter falsy entries out instead of inlining the conditionals.
                [
                  // Always present, never a look: render/scene_depth.ts
                  // publishes the world's depth for materials that have to ask
                  // whether a world point is visible. Not folded into N8AO's
                  // depth requirement on purpose — AO is expected to go once
                  // shading is baked into the textures.
                  <primitive key="scene_depth" object={scene_depth_pass} />,
                  show_ao && (
                    <N8AO
                      key="ao"
                      quality={post_cfg.AO.QUALITY}
                      halfRes={post_cfg.AO.HALF_RES}
                      aoRadius={post_cfg.AO.RADIUS}
                      intensity={post_cfg.AO.INTENSITY}
                      distanceFalloff={post_cfg.AO.DISTANCE_FALLOFF}
                      color={post_cfg.AO.COLOR}
                    />
                  ),
                  show_bloom && (
                    <Bloom
                      key="bloom"
                      intensity={post_cfg.BLOOM.INTENSITY}
                      luminanceThreshold={post_cfg.BLOOM.LUMINANCE_THRESHOLD}
                      luminanceSmoothing={post_cfg.BLOOM.LUMINANCE_SMOOTHING}
                      mipmapBlur={post_cfg.BLOOM.MIPMAP_BLUR}
                    />
                  ),
                  // sun_proxy_ref.current is only non-null once environment.tsx
                  // actually mounts the light-source mesh, which it only does
                  // while show_god_rays is on (same condition) — so this can't
                  // construct GodRaysEffect with a null sun the one frame
                  // between the setting flipping and the mesh's own ref
                  // attaching.
                  show_god_rays && sun_proxy_ref.current && (
                    <GodRays
                      key="god-rays"
                      sun={sun_proxy_ref.current}
                      samples={post_cfg.GOD_RAYS.SAMPLES}
                      density={post_cfg.GOD_RAYS.DENSITY}
                      decay={post_cfg.GOD_RAYS.DECAY}
                      weight={post_cfg.GOD_RAYS.WEIGHT}
                      exposure={post_cfg.GOD_RAYS.EXPOSURE}
                      clampMax={post_cfg.GOD_RAYS.CLAMP_MAX}
                      blendFunction={BlendFunction.SCREEN}
                    />
                  ),
                  <BrightnessContrast key="grade-bc" brightness={post_cfg.GRADE.BRIGHTNESS} contrast={post_cfg.GRADE.CONTRAST} />,
                  <HueSaturation key="grade-hs" saturation={post_cfg.GRADE.SATURATION} />,
                  <Vignette
                    key="vignette"
                    blendFunction={BlendFunction.NORMAL}
                    darkness={post_cfg.VIGNETTE.DARKNESS}
                    offset={post_cfg.VIGNETTE.OFFSET}
                  />,
                  // Last, so it cleans up after every other pass. A component,
                  // not an effect: it renders an SMAA effect or nothing, and
                  // the composer collects whichever it finds in the tree.
                  <AutoSmaa key="smaa" />,
                ].filter((effect): effect is JSX.Element => Boolean(effect))
              }
            </EffectComposer>
          )}
        </Canvas>
        {/* The WebGPU renderer's canvas, over three's until three goes. */}
        <GpuCanvas />
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

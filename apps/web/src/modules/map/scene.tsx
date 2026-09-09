// @pxl/web/modules/map/scene.tsx

import { type JSX, useEffect, useMemo, useRef } from "react";
import { Canvas, useThree } from "@react-three/fiber";
import * as THREE from "three";
import { Bloom, BrightnessContrast, ChromaticAberration, EffectComposer, GodRays, HueSaturation, N8AO, SMAA, TiltShift2, Vignette } from "@react-three/postprocessing";

import { SceneDepthPass } from "@/render/scene_depth.ts";
import { BlendFunction, SMAAPreset } from "postprocessing";

import { useCamera } from "@/modules/camera/use_camera.ts";
import { CameraInput } from "@/modules/camera/input.tsx";
import { camera_cfg } from "@/modules/camera/config.ts";
import { post_cfg, render_cfg } from "@/modules/world/environment/config.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { themes, type WorldTheme } from "@/modules/world/environment/themes.ts";
import { useThemeKey } from "@/modules/world/environment/theme_clock.ts";
import { FrameProvider } from "@/modules/world/frame/context.tsx";

import { MapEnvironment } from "./environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { useMapBootstrap } from "./navigation/use_map_bootstrap.ts";
import { useTileFetcher } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { usePoiFetcher } from "@/modules/world/poi/use_poi_fetcher.ts";
import { useGroundSnapSync } from "@/modules/world/terrain/use_ground_snap_sync.ts";
import { Overlays } from "./ui/overlay/overlays.tsx";
import { MapHTMLPortal } from "./portal/html_portal.tsx";

import { ToastController } from "./ui/toast/toast_controller.tsx";
import { LedgerOverlay } from "@/modules/world/sched/ledger_overlay.tsx";

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
  const show_tilt_shift = useSettingsStore((state) => state.show_tilt_shift);
  const show_chromatic_aberration = useSettingsStore((state) => state.show_chromatic_aberration);
  const show_god_rays = useSettingsStore((state) => state.show_god_rays);
  const aa_mode = useSettingsStore((state) => state.aa_mode);
  const show_shadow_map = useSettingsStore((state) => state.show_shadow_map);

  // ChromaticAberrationEffect's offset setter expects a real THREE.Vector2
  // (it copies .x/.y off the argument) — unlike TiltShift2's own props,
  // this component is the library's generic wrapEffect() wrapper, which
  // passes props straight through to the constructor with no array
  // coercion, so a plain [x, y] tuple would silently produce {x: undefined,
  // y: undefined}.
  const chromatic_aberration_offset = useMemo(
    () => new THREE.Vector2(...post_cfg.CHROMATIC_ABERRATION.OFFSET),
    [],
  );

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
          // dpr [1,2] when aa_mode is "supersample" (settings_store.ts,
          // default): real spatial supersampling, not just edge AA. MSAA
          // (tried below, previously multisampling={4}) only resolves
          // per-sample *coverage/depth* — the fragment shader itself still
          // runs once per output pixel, so it never re-evaluates shading at
          // sub-pixel positions. That's fine for a triangle's silhouette
          // edge, but this terrain is thousands of independently flat-shaded
          // boxes that minify well below one pixel at any real distance (see
          // the EffectComposer comment below) — the visible artifact there
          // is shading-value flicker between neighboring facets sharing a
          // pixel, which MSAA structurally cannot fix. Rendering the whole
          // canvas (WebGL draw + the full post stack) at up to 2x linear
          // resolution and letting the browser's own canvas-to-CSS
          // downscale do a real per-pixel color average fixes both classes
          // of aliasing at once — but costs roughly 4x the fragment-shading
          // work of the whole pipeline (post stack included), since dpr=2 on
          // a Retina/high-DPI display means 4x the pixels; separate from
          // chunk-count/frustum-culling effects, which don't depend on
          // pixel count at all. Confirmed the single biggest post-stack cost
          // on real devices, but the app now ships with a real art pack
          // behind it and defaults to this anyway — aa_mode's other two
          // options ("none", "smaa") are the Quality section's opt-out for
          // low-end/non-Retina devices, where this [1,2] clamp is a no-op
          // in the first place (clamp(1, 1, 2) = 1) and SMAA is the only
          // one of the two that does anything.
          // The [1,2] cap keeps it from ever exceeding 2x even on a
          // 3x/4x-DPR phone once it is.
          dpr={aa_mode === "supersample" ? [1, 2] : [1, 1]}
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
          <DevRenderToggleInvalidator deps={[show_ao, show_bloom, show_tilt_shift, show_chromatic_aberration, show_god_rays, aa_mode, show_shadow_map]} />
          <FrameProvider>
            <CameraRig />

            {/* Lighting and sky environment */}
            <MapEnvironment sun_proxy_ref={sun_proxy_ref} />

            {/* World layers in the floating-origin local frame */}
            <World ref={world_ref} />
          </FrameProvider>

          {
            /*
            "Miniature diorama" post stack — N8AO/Bloom/TiltShift2/
            ChromaticAberration/SMAA all default OFF, but not all for the
            same reason. The umbrella reason is mobile: none of these are
            things a Minecraft-default-options level would even offer as a
            toggle (SSAO/bloom/tilt-shift/lens fringing/post-AA are
            shader-pack territory, not vanilla settings). N8AO/Bloom/
            TiltShift2/SMAA additionally have a confirmed real cost from
            live profiling of the terrain pipeline (WebGLRenderer.
            renderBufferDirect/setProgram time scaling with the post
            stack) — ChromaticAberration wasn't part of that profiling run,
            it's off purely on the mobile-default-options principle above.
            All are user-reachable via the settings modal's Lighting/PostFx
            sections (app/settings/settings_store.ts's show_ao/show_bloom/
            show_tilt_shift/show_chromatic_aberration) rather than on by
            default — SMAA moved out to the Quality section's aa_mode radio
            (grouped with supersampling, not toggled independently here; see
            aa_mode's own doc comment for why). Order matters when these are on:
            1. N8AO — real screen-space contact/crevice AO. See git history
               (this comment block, pre-2026-08-13) for the normal-pass/
               radius-tuning notes from when it was built.
            2. Bloom — glow on the few genuinely bright things (sun disc, night
               lights); the high luminance threshold keeps daytime scenes from
               washing out.
            3. Color grade (contrast/saturation) — the flat/raw output reads
               washed out next to the punchy reference stills; this is cheap
               and global, stays active unconditionally below.
            4. TiltShift2 — screen-space blur band, the actual "macro lens"
               illusion that sold the miniature look. Worked because this
               camera holds a fixed top-down-ish angle, so screen-Y already
               correlates with depth the way a real tilt-shift lens exploits.
            5. Vignette — subtle framing; cheap, stays active unconditionally
               below.
            6. ChromaticAberration — small radial RG/B channel split, a lens-
               fringing cue toward the frame edge. Placed after Vignette/
               TiltShift2 (so it displaces already-graded/blurred color) and
               before SMAA (so AA smooths the fringe edges it introduces,
               same reasoning as running SMAA last overall).
            7. SMAA last — edge cleanup after every other pass ran.
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
                show_tilt_shift && (
                  <TiltShift2
                    key="tilt-shift"
                    blur={post_cfg.TILT_SHIFT.BLUR}
                    taper={post_cfg.TILT_SHIFT.TAPER}
                    start={post_cfg.TILT_SHIFT.START}
                    end={post_cfg.TILT_SHIFT.END}
                    direction={post_cfg.TILT_SHIFT.DIRECTION}
                    samples={post_cfg.TILT_SHIFT.SAMPLES}
                  />
                ),
                <Vignette
                  key="vignette"
                  blendFunction={BlendFunction.NORMAL}
                  darkness={post_cfg.VIGNETTE.DARKNESS}
                  offset={post_cfg.VIGNETTE.OFFSET}
                />,
                show_chromatic_aberration && (
                  <ChromaticAberration
                    key="chromatic-aberration"
                    offset={chromatic_aberration_offset}
                    radialModulation={post_cfg.CHROMATIC_ABERRATION.RADIAL_MODULATION}
                    modulationOffset={post_cfg.CHROMATIC_ABERRATION.MODULATION_OFFSET}
                  />
                ),
                // preset defaults to SMAAPreset.MEDIUM if unset — bumped to
                // HIGH for more edge-detection sensitivity (catches more/
                // subtler edges), a moderate step up rather than ULTRA.
                // aa_mode === "smaa", not a standalone show_smaa boolean:
                // mutually exclusive with supersampling (dpr [1,2] above) —
                // see aa_mode's own doc comment for why running both wastes
                // GPU budget for essentially no extra sharpness.
                aa_mode === "smaa" && <SMAA key="smaa" preset={SMAAPreset.HIGH} />,
              ].filter((effect): effect is JSX.Element => Boolean(effect))
            }
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

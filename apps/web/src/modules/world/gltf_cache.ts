// @pxl/web/modules/world/gltf_cache.ts

/**
 * Shared glTF/texture loading + caching, factored out of
 * characters/use_character_model.ts so any future GLB consumer (companion
 * models, decor/scatter instances, ...) hits the network/parser at most
 * once per URL instead of each owning its own cache. Deliberately plain
 * Promise caches, not drei's Suspense-integrated useGLTF/useTexture — see
 * use_character_model.ts's own doc comment for why Suspense's retry-from-
 * scratch semantics broke under a fast-re-rendering parent here.
 */

import * as THREE from "three";
import { GLTFLoader } from "three-stdlib";

// A Kenney glb names its colormap by a relative uri ("Textures/colormap.png")
// that the loader would resolve beside the model — under the content key,
// where nothing of that name exists. Every material is replaced with the
// skin the palette names anyway, so the reference is answered with a
// one-pixel image instead of a request: the loader gets its texture, the
// edge gets no 404 to answer.
const BLANK_PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
const manager = new THREE.LoadingManager();
manager.setURLModifier((url) => /\.(png|jpe?g|webp)$/i.test(url) ? BLANK_PNG : url);
const gltf_loader = new GLTFLoader(manager);

const gltf_cache = new Map<string, Promise<{ scene: THREE.Group; animations: THREE.AnimationClip[] }>>();

export function load_gltf(url: string): Promise<{ scene: THREE.Group; animations: THREE.AnimationClip[] }> {
  let entry = gltf_cache.get(url);
  if (!entry) {
    entry = new Promise((resolve, reject) => {
      gltf_loader.load(url, (gltf) => resolve({ scene: gltf.scene, animations: gltf.animations }), undefined, reject);
    });
    gltf_cache.set(url, entry);
  }
  return entry;
}

const texture_cache = new Map<string, Promise<THREE.Texture>>();
const texture_loader = new THREE.TextureLoader();

export function load_texture(url: string): Promise<THREE.Texture> {
  let entry = texture_cache.get(url);
  if (!entry) {
    entry = texture_loader.loadAsync(url);
    texture_cache.set(url, entry);
  }
  return entry;
}

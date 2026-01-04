// packages/core/src/shared/bootstrap/engine/types.ts

export interface EngineModule {
  name: string;
  init: () => Promise<void>;
}

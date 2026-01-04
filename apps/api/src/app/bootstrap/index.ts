import { ComputeEngine } from './compute.engine';

export const bootstrapEngines = async () => {
  const engines = [ComputeEngine];

  for (const engine of engines) {
    try {
      await engine.init();
    } catch (error) {
      console.error(
        `❌ [App Bootstrap] Failed to initialize ${engine.name}:`,
        error,
      );
      throw error;
    }
  }
};

import * as tf from '@tensorflow/tfjs-core';
import '@tensorflow/tfjs-backend-webgpu';

import { EngineModule } from '@pixelet/shared/shared/bootstrap/engine/types';

import { runComputeDiagnostics } from '@pixelet/shared/shared/bootstrap/specs/compute.test';

export const ComputeEngine: EngineModule = {
  name: 'ComputeEngine (WebGPU)',
  init: async () => {
    // 1. WebGPU 백엔드 설정
    await tf.setBackend('webgpu');
    await tf.ready();

    console.log(`⚙️  ${tf.getBackend()} backend ready.`);

    // 2. 인프라 진단 (api와 동일한 테스트 로직 공유 가능)
    if (import.meta.env.DEV) {
      runComputeDiagnostics();
    }
  },
};

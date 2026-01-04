import * as tf from '@tensorflow/tfjs-core';
import '@tensorflow/tfjs-node';

import { EngineModule } from '@pixelet/shared/shared/bootstrap/engine/types';

import { runComputeDiagnostics } from '@pixelet/shared/shared/bootstrap/specs/compute.test';

export const ComputeEngine: EngineModule = {
  name: 'ComputeEngine (TFJS)',
  init: async () => {
    await tf.ready();

    console.log(`⚙️  ${tf.getBackend()} backend ready.`);

    // 인프라 검증을 위한 테스트 실행
    if (process.env.NODE_ENV === 'development') {
      runComputeDiagnostics();
    }
  },
};

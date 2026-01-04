import * as tf from '@tensorflow/tfjs-core';

/**
 * Validates TFJS infrastructure and vector math capabilities.
 * Simulates Floating Origin transformation.
 */
export const runComputeDiagnostics = () => {
  console.log('🧪 Starting ComputeEngine Diagnostics...');

  tf.tidy(() => {
    // Local coordinates in a 16x16x16 chunk space
    const localCoords = tf.tensor2d(
      [
        [1.0, 0.0, 1.0],
        [8.0, 4.0, 8.0],
      ],
      [2, 3],
    );

    // Simulated Floating Origin offset
    const worldOffset = tf.tensor1d([100.0, 0.0, 100.0]);

    // Vectorized addition for origin shift
    const transformed = tf.add(localCoords, worldOffset);

    console.log('--- Transformation Result (Floating Origin) ---');
    transformed.print();
  });

  const { numTensors } = tf.memory();
  console.log(`🧹 Diagnostics complete. Active tensors: ${numTensors}`);
};

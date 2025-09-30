export {};

const CHUNK_SIZE = 100; // generate 100k points per batch

self.onmessage = (event) => {
  const { pointCount } = event.data;
  let generated = 0;

  while (generated < pointCount) {
    const batchSize = Math.min(CHUNK_SIZE, pointCount - generated);
    const positions = new Float32Array(batchSize * 3);
    const colorIndices = new Uint8Array(batchSize);

    for (let i = 0; i < batchSize; i++) {
      positions[i * 3] = (Math.random() - 0.5) * 200;
      positions[i * 3 + 1] = Math.random() * 10;
      positions[i * 3 + 2] = (Math.random() - 0.5) * 200;
      colorIndices[i] = 0;
    }

    // send chunk back to main thread
    postMessage({ positions, colorIndices }, [positions.buffer, colorIndices.buffer]);

    generated += batchSize;
  }

  // notify main thread that generation is finished
  postMessage({ done: true });
};

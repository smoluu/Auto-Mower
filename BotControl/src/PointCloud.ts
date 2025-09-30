import * as THREE from "three";
import { pointMaterial } from "./assets/materials";

export class PointCloud {
  public geometry: THREE.BufferGeometry;
  public material: THREE.ShaderMaterial;
  public points: THREE.Points;

  public debugWorker: Worker;

  private positions: Float32Array;
  private colorIndices: Uint8Array;
  private pointCount: number;

  private allocatedSize: number;

  constructor() {
    // Pre-allocate arrays for positions & color indecies
    this.allocatedSize = 10_000_000;
    this.positions = new Float32Array(this.allocatedSize * 3);
    this.colorIndices = new Uint8Array(this.allocatedSize);
    this.pointCount = 0;

    this.geometry = new THREE.BufferGeometry();
    this.geometry.setDrawRange(0, this.pointCount); // Only draw the first N points
    this.geometry.setAttribute("position", new THREE.Float32BufferAttribute(this.positions, 3));
    this.geometry.setAttribute("colorIndex", new THREE.Uint8BufferAttribute(this.colorIndices, 1));
    this.geometry.getAttribute("position").needsUpdate = true;
    this.geometry.getAttribute("colorIndex").needsUpdate = true;
    this.material = pointMaterial;

    this.points = new THREE.Points(this.geometry, this.material);
    this.pointCount = 0;

    // Worker
    this.debugWorker = new Worker(new URL("./workers/debug-worker.js", import.meta.url), {
      type: "module",
    });

    this.debugWorker.onmessage = (event) => {
      const data = event.data;
      if (data.done) {
        console.log("Point generation finished");
        return;
      }
      const positions = new Float32Array(data.positions);
      const colorIndices = new Uint8Array(data.colorIndices);
      this.addPoints(positions, colorIndices);
    };
  }

  addPoints(positions: Float32Array, colorIndices: Uint8Array) {
    const numNewPoints = positions.length / 3;
    const positionAttr = this.geometry.getAttribute("position") as THREE.BufferAttribute;
    const colorAttr = this.geometry.getAttribute("colorIndex") as THREE.BufferAttribute;

    // Write at the current pointCount
    positionAttr.array.set(positions, this.pointCount * 3);
    colorAttr.array.set(colorIndices, this.pointCount);

    this.pointCount += numNewPoints;
    this.geometry.setDrawRange(0, this.pointCount);

    positionAttr.needsUpdate = true;
    colorAttr.needsUpdate = true;

    console.log("added points");
  }
}

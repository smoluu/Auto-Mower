// Create 1D texture for color palette
import {ShaderMaterial, DataTexture, RGBAFormat} from "three";

export const colorPalette = new Uint8Array([
  255,255,255,255,
  255, 0, 0, 255,   // Red
  0, 255, 0, 255,   // Green
  0, 0, 255, 255,   // Blue
  255, 255, 0, 255, // Yellow
]);


const paletteTexture = new DataTexture(colorPalette, 5, 1, RGBAFormat);
paletteTexture.needsUpdate = true;

// Custom shader material for smooth, circular points with color palette
export const pointMaterial = new ShaderMaterial({
  uniforms: {
    pointSize: { value: 0.05 },
    palette: { value: paletteTexture },
  },
  vertexShader: `
      precision highp float;
      attribute float colorIndex;
      varying float vColorIndex;
      uniform float pointSize;
      void main() {
        vColorIndex = colorIndex / 5.0; // Normalize based on palette size (0 to 5)
        vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
        gl_PointSize = pointSize * (1000.0 / -mvPosition.z); // Adjusted scaling factor
        gl_Position = projectionMatrix * mvPosition;
      }
    `,
  fragmentShader: `
      precision highp float;
      uniform sampler2D palette;
      varying float vColorIndex;
      void main() {
        vec2 coord = gl_PointCoord - vec2(0.5);
        float dist = length(coord);
        if (dist > 0.5) discard; // Circular points
        gl_FragColor = texture2D(palette, vec2(vColorIndex, 0.5));
        gl_FragColor.a = 1.0; // Ensure full opacity for non-discarded pixels
      }
    `,
});

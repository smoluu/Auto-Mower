import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { invoke } from "@tauri-apps/api/core";
import { Chart, ChartConfiguration, registerables } from "chart.js";

// Register Chart.js components
Chart.register(...registerables);


// Interfaces for Tauri invoke responses
interface SensorData {
  temp: number;
  speed: number;
  battery: number;
}

interface Settings {
  udpPort: number;
  rtspUrl: string;
  brushSize: number;
}

// Tab switching
const tabs = document.querySelectorAll<HTMLButtonElement>(".tab");
const tabContents = document.querySelectorAll<HTMLDivElement>(".tab-content");
tabs.forEach((tab) => {
  tab.addEventListener("click", () => {
    tabs.forEach((t) => t.classList.remove("active"));
    tabContents.forEach((c) => c.classList.remove("active"));
    tab.classList.add("active");
    const tabContent = document.getElementById(tab.dataset.tab!);
    if (tabContent) {
      tabContent.classList.add("active");
    }
  });
});

// Three.js setup for point cloud
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
const canvas = document.getElementById("point-cloud") as HTMLCanvasElement;
const renderer = new THREE.WebGLRenderer({ canvas });
renderer.setSize(window.innerWidth - 300, window.innerHeight - 40);
const controls = new OrbitControls(camera, renderer.domElement);
camera.position.z = 5;

// Generate test point cloud
interface PointCloudData {
  positions: Float32Array;
  colors: Float32Array;
}

function generateTestPointCloud(pointCount: number = 100000): PointCloudData {
  const positions = new Float32Array(pointCount * 3);
  const colors = new Float32Array(pointCount * 3);
  for (let i = 0; i < pointCount; i++) {
    positions[i * 3] = (Math.random() - 0.5) * 10; // x: [-5, 5]
    positions[i * 3 + 1] = (Math.random() - 0.5) * 10; // y: [-5, 5]
    positions[i * 3 + 2] = (Math.random() - 0.5) * 10; // z: [-5, 5]
    colors[i * 3] = Math.random(); // r: [0, 1]
    colors[i * 3 + 1] = Math.random(); // g: [0, 1]
    colors[i * 3 + 2] = Math.random(); // b: [0, 1]

  }
  return { positions, colors };
}

// Load point cloud
let points: THREE.Points;
function loadPointCloud(): void {
  const cloud = generateTestPointCloud();
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(cloud.positions, 3));
  geometry.setAttribute("color", new THREE.Float32BufferAttribute(cloud.colors, 3));
  const material = new THREE.PointsMaterial({ size: 0.05, vertexColors: true });
  points = new THREE.Points(geometry, material);
  scene.add(points);
}
loadPointCloud();

// Animation loop
function animate(): void {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();

// Dynamic resize
function resizeCanvas(): void {
  const mainView = document.getElementById("main-view") as HTMLDivElement;
  const tabs = document.getElementById("tabs") as HTMLDivElement;
  const width = mainView.clientWidth;
  const height = mainView.clientHeight - tabs.clientHeight;
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  renderer.setSize(width, height);
}
window.addEventListener("resize", resizeCanvas);
resizeCanvas(); // Initial resize

// Sensor data update
async function updateSensors(): Promise<void> {
  const data = await invoke<SensorData>("get_sensor_data");
  const tempElement = document.getElementById("temp") as HTMLSpanElement;
  const speedElement = document.getElementById("speed") as HTMLSpanElement;
  const batteryElement = document.getElementById("battery") as HTMLSpanElement;
  tempElement.textContent = `${data.temp} °C`;
  speedElement.textContent = `${data.speed} m/s`;
  batteryElement.textContent = `${data.battery} %`;

  // Update battery gauge
  const ctx = (document.getElementById("battery-gauge") as HTMLCanvasElement).getContext("2d")!;
  new Chart(ctx, {
    type: "bar",
    data: {
      labels: ["Battery"],
      datasets: [{ label: "Level", data: [data.battery], backgroundColor: "rgba(74, 222, 128, 0.5)" }],
    },
    options: { scales: { y: { beginAtZero: true, max: 100 } } },
  } as ChartConfiguration);
}
setInterval(updateSensors, 1000);

// Painting functionality
let paintMode: boolean = false;
const raycaster = new THREE.Raycaster();
const mouse = new THREE.Vector2();
canvas.addEventListener("mousedown", async (event: MouseEvent) => {
  if (!paintMode) return;
  mouse.x = ((event.clientX - 300) / (window.innerWidth - 300)) * 2 - 1;
  mouse.y = -((event.clientY - 40) / (window.innerHeight - 40)) * 2 + 1;
  raycaster.setFromCamera(mouse, camera);
  const intersects = raycaster.intersectObject(points);
  if (intersects.length > 0) {
    const hit = intersects[0].point;
    const brushSize = parseFloat((document.getElementById("brush-size") as HTMLInputElement).value);
    const indices = await invoke<number[]>("select_points", { x: hit.x, y: hit.y, z: hit.z, radius: brushSize });
    const geometry = points.geometry as THREE.BufferGeometry;
    const colors = geometry.attributes.color.array as Float32Array;
    indices.forEach((i) => {
      colors[i * 3] = 0; // r
      colors[i * 3 + 1] = 1; // g
      colors[i * 3 + 2] = 0; // b
    });
    geometry.attributes.color.needsUpdate = true;
  }
});

const paintBtn = document.getElementById("paint-btn") as HTMLButtonElement;
paintBtn.addEventListener("click", () => {
  paintMode = !paintMode;
  paintBtn.textContent = paintMode ? "Exit Paint Mode" : "Toggle Paint Mode";
});

const saveAreasBtn = document.getElementById("save-areas-btn") as HTMLButtonElement;
saveAreasBtn.addEventListener("click", async () => {
  await invoke("save_marked_areas");
  alert("Allowed areas sent to robot");
});

// Settings save
// const saveSettingsBtn = document.getElementById("save-settings") as HTMLButtonElement;
// saveSettingsBtn.addEventListener("click", async () => {
//   const settings: Settings = {
//     udpPort: parseInt((document.getElementById("udp-port") as HTMLInputElement).value),
//     rtspUrl: (document.getElementById("rtsp-url") as HTMLInputElement).value,
//     brushSize: parseFloat((document.getElementById("brush-size") as HTMLInputElement).value),
//   };
//   await invoke("save_settings", settings);
//   alert("Settings saved");
// });

// RTSP feed (placeholder)
const rtspVideo = document.getElementById("rtsp-feed") as HTMLVideoElement;
const cameraVideo = document.getElementById("camera-feed") as HTMLVideoElement;
rtspVideo.src = "http://localhost:8080/stream"; // Adjust to WebRTC endpoint
cameraVideo.src = "http://localhost:8080/stream";

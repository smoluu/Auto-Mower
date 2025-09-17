import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { invoke } from "@tauri-apps/api/core";
import Stats from "stats.js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Chart, ChartConfiguration, registerables } from "chart.js";
import { PointCloud } from "./PointCloud";

// Register Chart.js components
Chart.register(...registerables);

let isFocus = true;
let stopRender = false;
let clock = new THREE.Clock();
const targetFPS  = 120;
const blurTargetFps = 15; // Used to limit renderer fps when app is not in focus
let rendererTargetFps = targetFPS; // Used to limit renderer frames per second
let delta = 0; // Renderer frame time

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

// Three.js setup
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(90, window.innerWidth / window.innerHeight, 0.1, 1000);
const canvas = document.getElementById("point-cloud") as HTMLCanvasElement;
const renderer = new THREE.WebGLRenderer({ canvas });
console.log(renderer.capabilities);
renderer.setSize(1080, 720);

// Camera setup
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.1;
controls.rotateSpeed = 0.5;
controls.panSpeed = 0.5;
camera.position.y = 20;
camera.position.z = -5;

// Axis  & Grid helper
const axesHelper = new THREE.AxesHelper(999);
scene.add(axesHelper);
scene.add(new THREE.GridHelper(999 , 999));

//  Stats setup
const stats = new Stats();
stats.showPanel(0); // 0: FPS, 1: MS, 2: MB
document.body.appendChild(stats.dom);
stats.dom.style.position = "absolute";
stats.dom.style.top = (window.innerHeight - 50).toString() + "px";
stats.dom.style.left = "0px";


// Load point cloud
const pointCloud = new PointCloud();
scene.add(pointCloud.points);
pointCloud.debugWorker.postMessage({ pointCount: 1_000_000 });

// App not in focus
getCurrentWindow().listen("tauri://blur", () => {
  isFocus = false;
  rendererTargetFps = blurTargetFps;
});
// App in focus
getCurrentWindow().listen("tauri://focus", () => {
  isFocus = true;
  rendererTargetFps = targetFPS;
});

function Update() {
  requestAnimationFrame(Update);

  delta += clock.getDelta();
  const interval = 1 / targetFPS;

  while (delta >= interval) {
    if (tabs[0]?.classList.contains("active")) {
      controls.update();
      stats.begin();
      renderer.render(scene, camera);
      stats.end();

      if (renderer.info.render.frame % rendererTargetFps === 0) {
        console.log(renderer.info.render);
        console.log(renderer.info.memory);
      }
    }

    delta -= interval; // keep leftover
  }
}
Update();

// Dynamic resize
function resizeCanvas(): void {
  const mainView = document.getElementById("main-view") as HTMLDivElement;
  const tabs = document.getElementById("tabs") as HTMLDivElement;
  const width = mainView.clientWidth;
  const height = mainView.clientHeight - tabs.clientHeight;
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  renderer.setSize(width, height);
  stats.dom.style.top = (mainView.clientHeight - 50).toString() + "px";
}

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

// Painting functionality
let paintMode: boolean = false;
const raycaster = new THREE.Raycaster();
raycaster.params.Points.threshold = 1;
const mouse = new THREE.Vector2();
canvas.addEventListener("mousedown", async (event: MouseEvent) => {

  console.log(pointCloud)

  if (!paintMode) return;
  let time = performance.now();
  const rect = canvas.getBoundingClientRect();
  mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
  mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
  raycaster.setFromCamera(mouse, camera);

  // Add cylinder for debug
  const material = new THREE.MeshBasicMaterial({
    color: 0x00ff00, // Green color
    transparent: true,
    opacity: 0.5,
    depthWrite: false, // Disable depth writing to make it appear transparent
  });
  const geometry = new THREE.CylinderGeometry(0.1, 0.1, 100, 16);

  const cylinder = new THREE.Mesh(geometry, material);
  let hitPoint = raycaster.ray.origin.clone().add(raycaster.ray.direction.clone().multiplyScalar(110));
  const direction = hitPoint.clone().sub(camera.position).normalize();
  const up = new THREE.Vector3(0, 1, 0);
  const quaternion = new THREE.Quaternion().setFromUnitVectors(up, direction);
  cylinder.quaternion.copy(quaternion);
  cylinder.position.lerpVectors(camera.position, hitPoint, 0.5);
  scene.add(cylinder);
  setTimeout(() => {
    scene.remove(cylinder);
    geometry.dispose();
    material.dispose();
  }, 10000);

  const intersects = raycaster.intersectObject(pointCloud.points, false);
  if (intersects.length > 0) {
    const hit = intersects[0];
    console.log(hit);
  }
  console.log("Raycast took: ", performance.now() - time);
});

const paintBtn = document.getElementById("paint-btn") as HTMLButtonElement;
paintBtn.addEventListener("click", () => {
  paintMode = !paintMode;
  controls.enableRotate = !paintMode
  paintBtn.textContent = paintMode ? "Exit Paint Mode" : "Toggle Paint Mode";
});

const saveAreasBtn = document.getElementById("save-areas-btn") as HTMLButtonElement;
saveAreasBtn.addEventListener("click", async () => {
  await invoke("save_marked_areas");
  alert("Allowed areas sent to robot");
});
// Keyboard mappings
window.addEventListener("keypress", (e) => {
  switch (e.key) {
    case "p":
      stopRender = !stopRender;
    case "d":
    pointCloud.debugWorker.postMessage({ pointCount: 10_000 });

  }
});

// Settings save
const saveSettingsBtn = document.getElementById("save-settings") as HTMLButtonElement;
saveSettingsBtn.addEventListener("click", async () => {
  const settings: Settings = {
    udpPort: parseInt((document.getElementById("udp-port") as HTMLInputElement).value),
    rtspUrl: (document.getElementById("rtsp-url") as HTMLInputElement).value,
    brushSize: parseFloat((document.getElementById("brush-size") as HTMLInputElement).value),
  };
  //await invoke("save_settings", settings);
  alert("Settings saved");
});

// RTSP feed (placeholder)
const rtspVideo = document.getElementById("rtsp-feed") as HTMLVideoElement;
const cameraVideo = document.getElementById("camera-feed") as HTMLVideoElement;
rtspVideo.src = "http://localhost:8080/stream"; // Adjust to WebRTC endpoint
cameraVideo.src = "http://localhost:8080/stream";

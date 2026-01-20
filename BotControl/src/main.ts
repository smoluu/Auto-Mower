import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/Addons.js";
import { invoke } from "@tauri-apps/api/core";
import Stats from "stats.js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Chart, ChartConfiguration, registerables } from "chart.js";
import { PointCloud } from "./PointCloud";
import { Toast } from "./Toast";
import { listen } from "@tauri-apps/api/event";
import { load, Store } from "@tauri-apps/plugin-store";

// Register Chart.js components
Chart.register(...registerables);

// GLTF Loader
const loader = new GLTFLoader();

let isFocus = true;
let stopRender = false;
let clock = new THREE.Clock();
const targetFPS = 30;
const blurTargetFps = 15; // Used to limit renderer fps when app is not in focus
let rendererTargetFps = targetFPS; // Used to limit renderer frames per second
let delta = 0; // Renderer frame time
let connectionState: "disconnected" | "connecting" | "connected" = "disconnected";
const connectionStatusDot = document.querySelector("#connectionStatusDot") as HTMLDivElement;
const connectButton = document.querySelector("#device-connect-btn") as HTMLDivElement;

enum CanvasTool {
  None = "none",
  Paint = "paint",
  Erase = "erase",
  Select = "select",
}
let canvasTool: CanvasTool = CanvasTool.None;
// Interfaces for Tauri invoke responses
interface SensorData {
  temp: number;
  speed: number;
  battery: number;
}

interface Settings {
  robotIp: string;
  robotPort: number;
  cameraURL: string;
}
export const DEFAULT_SETTINGS = {
  robotIp: "127.0.0.1",
  robotPort: 6969,
  cameraURL: "rtsp://localhost:8554",
} satisfies Settings;

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
    resizeCanvas();
  });
});

// Tool switching
const tools = document.querySelectorAll<HTMLDivElement>(".tool-btn");
tools.forEach((tool) => {
  tool.addEventListener("click", () => {
    let active = tool.classList.contains("active");
    tools.forEach((t) => {
      t.classList.remove("active");
    });
    let toolType = tool.dataset.tool as CanvasTool;
    if (active) {
      toolType = CanvasTool.None;
    }
    if (tool.id == "pointCloud-tool" && !active) {
      console.log(tool);
      tool.classList.add("active");
    }
    console.log(toolType);
    if (!toolType) return;

    // Disable all tools
    // paint
    controls.enableRotate = true;

    // Activate selected tool
    canvasTool = toolType;
    console.log(canvasTool);
    switch (canvasTool) {
      case CanvasTool.None:
        break;
      case CanvasTool.Paint: {
        controls.enableRotate = false;
        break;
      }
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
controls.minDistance = 0.1
controls.maxDistance = 100
controls.enableDamping = true;
controls.dampingFactor = 0.3;
controls.rotateSpeed = 0.5;
controls.panSpeed = 0.5;
camera.position.y = 1;
camera.position.z = -1;
scene.add(new THREE.AmbientLight(0xffffff, 1));

// Axis  & Grid helper
const axesHelper = new THREE.AxesHelper(999);
scene.add(axesHelper);
scene.add(new THREE.GridHelper(100, 100));

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

// Load Mower
loader.load("models/mower.glb", function(gltf) {
  const model = gltf.scene;

  model.scale.multiplyScalar(0.001);
  // Center model
  var box = new THREE.Box3().setFromObject(model);
  const center = box.getCenter(new THREE.Vector3());
  model.position.sub(center);

  scene.add(model);
});

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

// Render loop
function Update() {
  requestAnimationFrame(Update);

  delta += clock.getDelta();
  const interval = 1 / targetFPS;

  while (delta >= interval && !stopRender) {
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
  const targetAspectRatio = 16 / 9;
  const containerWidth = mainView.clientWidth;
  const containerHeight = mainView.clientHeight - tabs.clientHeight;

  // Option 1: fit by width
  let widthByWidth = containerWidth;
  let heightByWidth = Math.floor(widthByWidth / targetAspectRatio);

  // Option 2: fit by height
  let heightByHeight = containerHeight;
  let widthByHeight = Math.floor(heightByHeight * targetAspectRatio);

  let width: number;
  let height: number;

  if (heightByWidth <= containerHeight) {
    // Fits within height → use width-limited option
    width = widthByWidth;
    height = heightByWidth;
  } else {
    // Otherwise use height-limited option
    width = widthByHeight;
    height = heightByHeight;
  }
  camera.aspect = targetAspectRatio;
  camera.updateProjectionMatrix();
  renderer.setSize(width, height);

  stats.dom.style.top = (mainView.clientHeight - 50).toString() + "px";
}
resizeCanvas(); // Initial resize
window.addEventListener("resize", resizeCanvas);

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
const raycaster = new THREE.Raycaster();
raycaster.params.Points.threshold = 1;
const mouse = new THREE.Vector2();
canvas.addEventListener("mousedown", async (event: MouseEvent) => {
  switch (canvasTool) {
    case CanvasTool.None:
      break;
    case CanvasTool.Paint: {
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
    }
  }
});

const saveAreasBtn = document.getElementById("tools-tab-save-btn") as HTMLButtonElement;
saveAreasBtn.addEventListener("click", async () => {
  alert("Saved");
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

// Load Settings
const loadSettings = async () => {
  try {
    const storage = await load("storage.json");
    const savedSettings = await storage.get<Settings>("settings");
    if (savedSettings) {
      // Populate your form inputs
      (document.getElementById("robot-ip") as HTMLInputElement).value = savedSettings.robotIp;
      (document.getElementById("robot-port") as HTMLInputElement).value = savedSettings.robotPort.toString();
      (document.getElementById("camera-url") as HTMLInputElement).value = savedSettings.cameraURL;
      Toast.new("Settings loaded"); // Using your fixed Toast
    } else {
      // Save defaults
      await storage.set("settings", DEFAULT_SETTINGS);
    }
  } catch (error) {
    console.error("Failed to load settings:", error);
    Toast.new("Failed to load settings", "error");
  }
};
document.addEventListener("DOMContentLoaded", loadSettings);

const saveSettingsBtn = document.getElementById("save-settings") as HTMLButtonElement;
saveSettingsBtn.addEventListener("click", async () => {
  const settings: Settings = {
    robotIp: (document.getElementById("robot-ip") as HTMLInputElement).value,
    robotPort: parseInt((document.getElementById("robot-port") as HTMLInputElement).value),
    cameraURL: (document.getElementById("camera-url") as HTMLInputElement).value,
  };

  try {
    const storage = await load("storage.json");
    await storage.set("settings", settings);
    await storage.save();
    Toast.new("Settings saved successfully!", "success");
    console.log("", await storage.get("settings"));
  } catch (error) {
    console.error("Failed to save settings:", error);
    Toast.new("Failed to save settings", "error");
  }
});

// RTSP feed (placeholder)
const rtspVideo = document.getElementById("rtsp-feed") as HTMLVideoElement;
const cameraVideo = document.getElementById("camera-feed") as HTMLVideoElement;
rtspVideo.src = "http://localhost:8080/stream"; // Adjust to WebRTC endpoint
cameraVideo.src = "http://localhost:8080/stream";

const deviceConnectButton = document.querySelector("#device-connect-btn");
deviceConnectButton?.addEventListener("click", async () => {
  const storage = await load("storage.json");
  const settings = await storage.get<Settings>("settings");

  const dest = settings?.robotIp;
  const port = settings?.robotPort;
  try {
    if (connectionState == "disconnected") {
      let test = await invoke("connect_udp", { address: dest, port: port });
      console.log("test", test);
    }
  } catch (e) {
    console.log("Error connecting: ", e);
  }
});

listen("state_connection_update", (e) => {
  console.log(e.payload);
  const state = e.payload;
  if (state === "disconnected" || state === "connecting" || state === "connected") {
    connectionState = state;
    updateConnectionUI();
  } else {
    console.error("Invalid connection state:", e);
  }
});
function updateConnectionUI() {
  const colors = {
    disconnected: "#a80000",
    connecting: "#ffff00",
    connected: "#00c40aff",
  };
  const text = {
    disconnected: "Connect",
    connecting: "Disconnect",
    connected: "Disconnect",
  };
  connectionStatusDot.style.backgroundColor = colors[connectionState];
  connectButton.innerHTML = text[connectionState];
}

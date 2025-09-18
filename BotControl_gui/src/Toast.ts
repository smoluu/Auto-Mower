// toast.js — Vanilla JS Toast Notification System
export class Toast {
  static container: HTMLDivElement;
  constructor() {
  }

  static ensureContainer() {
    if (!Toast.container) {
      Toast.container = document.createElement("div");
      Toast.container.id = "toast-container";
      document.body.appendChild(Toast.container);

      const style = document.createElement("style");
      style.textContent = `
        #toast-container {
          position: fixed;
          bottom: 20px;
          right: 20px;
          display: flex;
          flex-direction: column;
          gap: 10px;
          z-index: 9999;
        }

        .toast {
          border: double;
          border-color: #473939ff;
          min-width: 200px;
          max-width: 300px;
          padding: 12px 16px;
          border-radius: 8px;
          color: white;
          font-family: sans-serif;
          font-size: 14px;
          box-shadow: 0 4px 10px rgba(0,0,0,0.3);
          opacity: 0;
          transform: translateY(20px);
          transition: opacity 0.3s ease, transform 0.3s ease;
        }

        .toast.show {
          opacity: 1;
          transform: translateY(0);
        }

        .toast.info { background: #272727; }   /* blue */
        .toast.success { background: #16a34a; }/* green */
        .toast.error { background: #dc2626; }  /* red */
        .toast.warning { background: #ca8a04; }/* yellow */
      `;
      document.head.appendChild(style);
    }
  }
/**
 * @param message String
 * @param type  "info", "error", "success", "warning"
 * @param duration in milliseconds
 */
  static new(message: string, type: string = "info", duration: number = 4000 ) {
    Toast.ensureContainer();

    const toast = document.createElement("div");
    toast.className = `toast ${type}`;
    toast.textContent = message;

    Toast.container.appendChild(toast);

    requestAnimationFrame(() => toast.classList.add("show"));

    setTimeout(() => {
      toast.classList.remove("show");
      toast.addEventListener("transitionend", () => toast.remove());
    }, duration);
  }
}
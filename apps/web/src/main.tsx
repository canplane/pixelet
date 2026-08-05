// @pxl/web/main.tsx

// TODO: i18n initialization (not wired up yet)

import { enableMapSet } from "immer";
import { bootstrap_app } from "@/bootstrap/app.ts"; // 엔진 부트스트랩 임포트

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/app/styles/index.css";
import { App } from "@/app/app.tsx";

enableMapSet(); // Immer의 Map/Set 지원 플러그인 활성화

let root: ReturnType<typeof createRoot> | null = null;

// 비동기 엔진 초기화 후 렌더링 시작
export function main(): void {
  try {
    // 1. 인프라 부트스트랩 (WebGPU 준비 및 Diagnostics 실행)
    bootstrap_app();

    // 2. UI 렌더링
    if (!root) {
      const container = document.getElementById("root");
      if (!container) throw new Error(`web: missing root container: #root`);
      root = createRoot(container);
    }

    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  } catch (err) {
    console.error("web: 🔥 failed to bootstrap engines:", err);
    // 필요 시 엔진 로드 실패 화면(Fallback) 처리
  }
}
// main();

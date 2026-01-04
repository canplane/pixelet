import '@pixelet/shared/shared/lib/i18n/i18n.client'; // i18n initialization
import { enableMapSet } from 'immer';
import { bootstrapEngines } from '@/app/bootstrap'; // 엔진 부트스트랩 임포트

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@/app/styles/index.css';
import { App } from '@/app/app';

enableMapSet(); // Immer의 Map/Set 지원 플러그인 활성화

const container = document.getElementById('root')!;
const root = createRoot(container);

// 비동기 엔진 초기화 후 렌더링 시작
async function startApp() {
  try {
    // 1. 인프라 부트스트랩 (WebGPU 준비 및 Diagnostics 실행)
    await bootstrapEngines();

    // 2. UI 렌더링
    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  } catch (error) {
    console.error('🔥 Failed to bootstrap engines:', error);
    // 필요 시 엔진 로드 실패 화면(Fallback) 처리
  }
}

startApp();

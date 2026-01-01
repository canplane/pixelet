import '@/shared/lib/i18n/i18n'; // i18n initialization

import { enableMapSet } from 'immer';

// Immer의 Map/Set 지원 플러그인 활성화
enableMapSet();

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@/app/styles/index.css';
import { App } from '@/app/app';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { bootstrapApp } from "@/app/bootstrap.ts";
import { AppLayout } from "@/app/layout.tsx";
import WorldPage from "@/page/world.tsx";

bootstrapApp();

export function App() {
  return (
    <StrictMode>
      <SafeAreaProvider>
        <AppLayout>
          <WorldPage />
        </AppLayout>
      </SafeAreaProvider>
    </StrictMode>
  );
}

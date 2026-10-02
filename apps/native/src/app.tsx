// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { bootstrapApp } from "@/app/bootstrap.ts";
import { AppLayout } from "@/app/layout.tsx";
import MapPage from "@/page/map.tsx";

bootstrapApp();

export function App() {
  return (
    <StrictMode>
      <SafeAreaProvider>
        <AppLayout>
          <MapPage />
        </AppLayout>
      </SafeAreaProvider>
    </StrictMode>
  );
}

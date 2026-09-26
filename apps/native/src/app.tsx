// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { bootstrapApp } from "@/app/bootstrap.ts";
import { AppLayout } from "@/app/layout.tsx";
import MapPage from "@/page/map-page.tsx";

bootstrapApp();

/** The map, as the web's one route shows it. A device has no URL to route
 * by, and the dev routes are the web's. */
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

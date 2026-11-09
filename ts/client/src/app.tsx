// @pxl/client/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { AppLayout } from "@/ui/layout.tsx";
import WorldScreen from "@/ui/feature/world/screen.tsx";

export function App() {
  return (
    <StrictMode>
      <SafeAreaProvider>
        <AppLayout>
          <WorldScreen />
        </AppLayout>
      </SafeAreaProvider>
    </StrictMode>
  );
}

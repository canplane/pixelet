// @pxl/client/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { AppLayout } from "@/ui/layout.tsx";
import WorldPage from "@/ui/world/page.tsx";

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

// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { AppLayout } from "@pxl/client/shell/layout.tsx";
import WorldPage from "@pxl/client/shell/world/page.tsx";

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

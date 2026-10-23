// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { setBracket } from "@pxl/client/bracket.tsx";
import { bootstrapApp } from "@pxl/client/shell/bootstrap.ts";
import { AppLayout } from "@pxl/client/shell/layout.tsx";
import WorldPage from "@pxl/client/shell/world/page.tsx";
import { DEVICE } from "@/bracket.ts";

setBracket(DEVICE);
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

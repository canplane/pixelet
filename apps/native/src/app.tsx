// @pxl/native/app.tsx

import { StrictMode } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import * as bracket from "@pxl/client/bracket.tsx";
import { boot } from "@pxl/client/shell/boot.ts";
import { AppLayout } from "@pxl/client/shell/layout.tsx";
import WorldPage from "@pxl/client/shell/world/page.tsx";
import { app } from "@/bracket.ts";

bracket.set(app);
boot();

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

// @pxl/client/app.tsx

import { StrictMode } from "react";
import { StyleSheet } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { KeyboardProvider } from "react-native-keyboard-controller";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { AppLayout } from "@/ui/layout.tsx";
import WorldScreen from "@/ui/feature/world/screen.tsx";

export function App() {
  return (
    <StrictMode>
      <GestureHandlerRootView style={StyleSheet.absoluteFill}>
        <KeyboardProvider>
          <SafeAreaProvider>
            <AppLayout>
              <WorldScreen />
            </AppLayout>
          </SafeAreaProvider>
        </KeyboardProvider>
      </GestureHandlerRootView>
    </StrictMode>
  );
}

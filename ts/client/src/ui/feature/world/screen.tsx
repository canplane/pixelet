// @pxl/client/ui/feature/world/screen.tsx

import { StyleSheet, View } from "react-native";

import { WorldView } from "@/ui/feature/world/view.tsx";

function WorldScreen() {
  return (
    <View style={STYLE.screen}>
      <WorldView />
    </View>
  );
}

const STYLE = StyleSheet.create({
  screen: { flex: 1, width: "100%", alignItems: "center" },
});

export default WorldScreen;

// @pxl/client/ui/world/screen.tsx

import { StyleSheet, View } from "react-native";

import { WorldView } from "@/ui/world/view.tsx";

function WorldScreen() {
  return (
    <View style={styles.screen}>
      <WorldView />
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, width: "100%", alignItems: "center" },
});

export default WorldScreen;

// @pxl/web/page/world.tsx

import { StyleSheet, View } from "react-native";

import { WorldView } from "@/feature/world/ui/view.tsx";

function WorldPage() {
  return (
    <View style={styles.page}>
      <WorldView />
    </View>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, width: "100%", alignItems: "center" },
});

export default WorldPage;

// @pxl/client/shell/world/page.tsx

import { StyleSheet, View } from "react-native";

import { WorldView } from "@/shell/world/view.tsx";

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

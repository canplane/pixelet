// @pxl/web/page/map.tsx

import { StyleSheet, View } from "react-native";

import { MapView } from "@/feature/map/ui/view.tsx";

function MapPage() {
  return (
    <View style={styles.page}>
      <MapView />
    </View>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, width: "100%", alignItems: "center" },
});

export default MapPage;

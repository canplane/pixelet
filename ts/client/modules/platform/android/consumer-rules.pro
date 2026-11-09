# What the core reaches by name through JNI (pxl-core sys/io/text_device.rs).
-keep class me.plei.pixelet.platform.PxlNative {
    public static double width(byte[], double, double);
    public static void raster(byte[], double, double, long, double, int, int, java.nio.ByteBuffer);
}

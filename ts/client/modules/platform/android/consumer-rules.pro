# What the runtime reaches by name through JNI (pxl-runtime text.rs).
-keep class me.plei.pixelet.platform.PxlNative {
    public static double width(byte[], double, double);
    public static void raster(byte[], double, double, long, double, int, int, java.nio.ByteBuffer);
}

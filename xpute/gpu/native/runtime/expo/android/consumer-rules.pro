# The static callback the GPU's host reaches by name through JNI, which it
# finds on the class `Gpu.init` is called on (Gpu.kt).
-keep class xpute.Gpu {
    public static void onLost();
}

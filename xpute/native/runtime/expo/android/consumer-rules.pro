# What the host reaches by name through JNI (native/host/src/jni.rs).
-keep class xpute.Runtime {
    public static int call(int);
    public static void onNotify();
}

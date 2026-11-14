# What the runtime reaches by name through JNI (native/runtime/rust/src/jni.rs).
-keep class xpute.Runtime {
    public static int call(int);
    public static void onNotify();
}

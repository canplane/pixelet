# What the runtime reaches by name through JNI (rust/runtime/src/jni.rs).
-keep class xpute.Core {
    public static int call(int);
    public static void onNotify();
}

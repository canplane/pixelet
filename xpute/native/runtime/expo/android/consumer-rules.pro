# The static callbacks the runtime reaches by name through JNI, which it finds
# on the class `Runtime.init` is called on (Runtime.kt).
-keep class xpute.Runtime {
    public static void onNotify();
    public static void onStopped(int, java.lang.String, int, long, long);
    public static void onTrap();
}

# The static callbacks the runtime reaches by name through JNI, which it finds
# on the class `Runtime.init` is called on (Runtime.kt).
-keep class xpute.Runtime {
    public static void onNotify();
    public static void onStopped(xpute.Stop);
    public static void onTrap(java.lang.String);
}
# The stop the runtime makes by name through JNI, for `onStopped` and `stop`.
-keep class xpute.Stop {
    public <init>(int, java.lang.String, int, long, long);
}

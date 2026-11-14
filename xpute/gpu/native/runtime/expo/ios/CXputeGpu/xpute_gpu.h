// @xpute/gpu-native-runtime/ios/CXputeGpu/xpute_gpu.h

// This is the C ABI of the GPU's host linked into the app beside xpute's
// runtime (xpute.h); xpute-gpu-native-host's `native_abi!` exports it. The
// host opens its device when the guest asks for one, and runs the device and
// the windows bound on a thread of its own, the GPU's. A device the system
// loses is let go, and the guest is told and asks for a device again; the
// next device binds every window bound. The app is told what the host has of
// a GPU as it changes (`xpute_gpu_state_on`).

#pragma once

#include <stddef.h>
#include <stdint.h>

// Makes `state` be told what the host has of a GPU, by its name's UTF-8
// bytes and their count, as `xpute_stopped_on`'s file is, and as a page's
// host reports it (@xpute/gpu-link `GpuState`): "present" once the guest's
// ask finds a device open, "missing" once it finds none opens, and "waiting"
// once a device is lost and the guest told, as it asks again. Null sets none.
// It is called on the GPU's thread and must not call into the GPU's host: it
// hands the news to the app's thread.
void xpute_gpu_state_on(void (*state)(const char *state, size_t len));

// Binds the guest's surface `handle` to `window`: on an Apple platform a
// view's CAMetalLayer, on Android a view's ANativeWindow, live at the call. The
// host holds a reference of its own on the window, retained or acquired,
// until the window is unbound. It binds on the GPU's thread and returns at
// once: 0, or -EINVAL (-22) for a null window. Where the system makes no
// surface of the window, or a newer handle has bound the slot since, it is
// logged and the window let go. While no device is open the window is kept,
// and the next device binds it.
int32_t xpute_gpu_surface_bind(uint32_t handle, void *window);

// Lets the window `handle` bound go: its surface, if `handle` is still the one
// bound in its slot, and the host's reference on the window in any case. It
// returns once the window is let go, so nothing draws into it after.
void xpute_gpu_surface_unbind(uint32_t handle);

// Tells the host of a display frame, from the platform's display link on any
// thread; it answers the guest's waiting `vsync.next`.
void xpute_gpu_vsync(void);

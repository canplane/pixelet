// @pxl/client/abi/cmd/command.spec.ts
//
// GENERATED from spec/client/kernel/domain/control/cmd.json

// ---- the process asks the host: its hostcalls, raised on the completion ring and run before the next turn (the rest are the host stream's records, host.json) ----

/** Start watching. */
export const SENSOR_START = 0x1201;
/** Stop watching. */
export const SENSOR_STOP = 0x1203;

// ---- the host answers: an I/O the process started, done ----

/** A GPS fix; a field is absent where the browser gave none. */
export const SENSOR_POS = 0x1202;
/** An orientation event; a field is absent where the browser gave none. */
export const SENSOR_ORIENT = 0x1205;
/** A watch that ended in failure and was stopped, refused from the start (PermissionDenied)
 * or cut off; the compass's, where the platform says so (SYS_BOOT's
 * orient_silence_is_loss). */
export const SENSOR_ERR = 0x120b;

// ---- the host tells the process: its start, its turn, its window and its devices ----

/** The kernel's start: what the platform is. Its storage files it opens itself, on the storage root. */
export const SYS_BOOT = 0x1001;
/** The frame of the turn the doorbell's quota rang in: the effects' turn, then the tick's
 * phases on what is left of the quota, the fetches within the credits. The turn leaves
 * its events on the completion ring, drawn by the kernel. It carries the host's alone:
 * the turn's pacing, the network's credits and the clock; the view it is drawn at is the
 * world surface's (SURFACE_RESIZE). */
export const SURFACE_FRAME = 0x1504;
/** The platform about to stop the process, or waking it: the page or the app hidden or
 * shown. The host rings the doorbell for it at once, since a hidden page gets no frame and
 * hiding is the save's last sure moment. */
export const SYS_SLEEP = 0x1002;
/** What the tile source declares of itself, once the client has it: the tile root's
 * `source`, the published one out of the bucket or the dev tile server's, read by the
 * client (ts/client/src/driver/namespace/source.ts) and handed over as numbers and names.
 * Until it comes nothing is observed: the floor is held at the zoom it names, and no
 * tile is asked for that no shard it serves is over or under (pxl-kernel sys/io/source.rs). */
export const FS_MOUNT = 0x1301;
/** A file the kernel refused (the snapshot's `refused_files`) is empty now, or its source's
 * own again, what it held set aside by the host: the kernel reads it again, and writes
 * a storage file from then on. */
export const FS_TRUNCATED = 0x1302;
/** A permission as the shell's prompt or the browser's change reported it. */
export const SENSOR_PERMISSION = 0x1206;
/** A device answered: the kernel asks its block (xpute DEVICE) and makes the renderer on it once the block is in (rust/client/render/src/gpu/api.rs). Its loss is the block's to say (xpute STATUS.LOST, written by the host's GpuHost), read at the turn's edge. */
export const GPU_READY = 0x1401;

// ---- the shell and the process: what the UI asks of it, the input it routes, and what it shows the UI ----

/** A pointer event on the world's canvas, in CSS pixels from the top left of its layout box
 * (transforms aside), as the world surface's size is (SURFACE_RESIZE). */
export const INPUT_POINTER = 0x1102;
/** The tool and what it takes, sent when it changes: the tool, the cube or decor a placing tool takes, and the object size tier. The kernel keeps a cube and a decor apart, so choosing another tool forgets neither, and picks again under a hovering pointer. */
export const EDIT_TOOL = 0x1a01;
/** The draft's text (ToolState.draft), which the marker is drawn with and committed with. Text past MARKER_TEXT_CHARS (pxl-spec object/ext.json) is refused whole, never cut. */
export const EDIT_MARKER_TEXT = 0x1a02;
/** The marker's draft dropped, nothing committed. */
export const EDIT_MARKER_CANCEL = 0x1a03;
/** The marker's draft committed with its text, where its place is still free or still a marker, and closed. */
export const EDIT_MARKER_COMMIT = 0x1a04;
export const INPUT_WHEEL = 0x1103;
/** The screen direction the held movement keys point, sent when it changes: each axis -1, 0 or 1, x to the right and y down. */
export const INPUT_MOVE = 0x1106;
/** A call, answered with the surface's handle, 0 refused: a canvas the shell hands over, drawn by the kernel alone from then on. */
export const SURFACE_OPEN = 0x1501;
/** The canvas's layout box in device pixels, transforms aside; the buffer it is drawn into is the kernel's. */
export const SURFACE_RESIZE = 0x1502;
/** A call: the canvas is going, and what it held on the device is let go before the call returns. */
export const SURFACE_CLOSE = 0x1503;
/** A character drawn alone into a surface the shell handed over, as the picker shows it. */
export const SCENE_PREVIEW_OPEN = 0x1701;
export const SCENE_PREVIEW_CLOSE = 0x1702;
/** A setting set from the sheet or the character picker; EAGAIN until the kernel has read the
 * kept settings, which would be read over it. */
export const SETTING_SET = 0x1601;
/** A line the player said, shown over the character. */
export const SCENE_CHAT = 0x1703;
/** The tracking button's press, as the button showed it. */
export const CAMERA_TRACKING = 0x1801;
/** The compass's press: north up. */
export const CAMERA_NORTH_UP = 0x1802;
/** The search panel's location card's press: the location asked for. The compass is
 * asked for where it is used, entering follow mode (TRACKING), and never alone: nothing
 * reads it without the location. */
export const SENSOR_LOCATE = 0x120c;
/** A search result: the pivot moved there, and the view asked with it at once, as the
 * controls ask it where the camera stands; a view field absent keeps what the view has. */
export const CAMERA_GOTO = 0x1804;
/** A tracked pose: where a tracker has the camera, and its turn as a unit quaternion
 * (x, y, z, w) from the camera's axes to the tracker's, in the world's units. The
 * first of a run is taken to be where the camera already is, so nothing moves as
 * it begins; the camera follows the tracker from there until POSE_END.
 * Nothing sends it yet: it is where an immersive mode's head pose (a Quest's, a Vision Pro's) would drive
 * the camera, sent by the device app's driver. */
export const INPUT_XR_POSE = 0x1109;
/** The tracker let the camera go: back to the controller it had. */
export const INPUT_XR_POSE_END = 0x110a;
/** A fresh recording window. */
export const PERF_START = 0x1901;
/** The window frozen, so what is exported holds still. */
export const PERF_STOP = 0x1902;
/** The profiler's statistics and what it holds now, where they lie: the reply is the address of STATS's words (perf.json), the statistics
 * over the last `window_ms`, the whole recording where it is absent, good until the next turn. A
 * number past the door is finite, so no window is infinite. */
export const PERF_STATS = 0x1903;
/** The rings where they lie, nothing reduced: the reply is the address of RINGS's words (perf.json), good until the next turn. */
export const PERF_RINGS = 0x1904;
/** The UI's snapshot, when it changed: a Snapshot packet far past a ring frame, so a
 * block the kernel holds, valid until the next snapshot. */
export const SHELL_SNAPSHOT = 0x1b01;

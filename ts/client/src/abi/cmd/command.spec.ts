// ts/client/src/abi/cmd/command.spec.ts
//
// GENERATED from rust/client/kernel/shell.json

// Every command, by the side that sends it.

// ---- shell, export ----

/** A GPS fix; a field is absent where the browser gave none. */
export const SENSOR_POS = 0x1202;
/** An orientation event; a field is absent where the browser gave none. */
export const SENSOR_ORIENT = 0x1205;
/** A watch that ended in failure and was stopped, refused from the start (PermissionDenied)
 * or cut off; the compass's, where the platform says so (SYS_BOOT's
 * orient_silence_is_loss). */
export const SENSOR_ERR = 0x120b;
/** The frame of the turn the doorbell rang: the effects' turn, then the tick's phases in
 * what is left before the deadline, the fetches within the credits. The turn leaves
 * its imports on the completion ring, drawn by the kernel. It carries the runtime's alone:
 * the turn's pacing, the network's credits and the clock; the view it is drawn at is the
 * world surface's (SURFACE_RESIZE). */
export const TURN_GRANT = 0x1d01;
/** What the tile source declares of itself, once the client has it: the tile root's
 * `source`, the published one out of the bucket or the dev tile server's, read by the
 * client (ts/client/src/sys/fs/source.ts) and handed over as numbers and names.
 * Until it comes nothing is observed: the floor is held at the zoom it names, and no
 * tile is asked for that no shard it serves is over or under (pxl-kernel sys/io/source.rs). */
export const FS_MOUNT = 0x1301;
/** A file the kernel refused (the snapshot's `refused_files`) is empty now, or its source's
 * own again, what it held set aside by the client: the kernel reads it again, and writes
 * a storage file from then on. */
export const FS_TRUNCATED = 0x1302;
/** A permission as the shell's prompt or the browser's change reported it. */
export const SENSOR_PERMISSION = 0x1206;
/** What the place's sensors are: whether it has a position and a heading, whether the
 * heading asks a permission, and how it says it is lost. Sent once the kernel is attached. */
export const SENSOR_PLATFORM = 0x120d;
/** Whether this is a development build, which offers the settings schema.json marks `dev`.
 * Sent once the kernel is attached. */
export const SETTING_DEVELOPMENT = 0x1602;
/** A pointer event on the world's canvas, in CSS pixels from the top left of its layout box
 * (transforms aside), as the world surface's size is (SURFACE_RESIZE). */
export const INPUT_POINTER = 0x1102;
/** The tool and what it takes, the shell's to choose and keep: the kernel is told, and answers EINVAL for an item the catalog does not give out. It picks again under a hovering pointer. */
export const EDIT_TOOL = 0x1a01;
/** The draft's text (Snapshot.draft), which the marker is drawn with and committed with. Text past MARKER_TEXT_CHARS (pxl-spec object/ext.json) is refused whole, never cut: EMSGSIZE, which the shell shows as what does not fit. */
export const EDIT_MARKER_TEXT = 0x1a02;
/** The marker's draft dropped, nothing committed. */
export const EDIT_MARKER_CANCEL = 0x1a03;
/** The marker's draft committed with its text, where its place is still free or still a marker, and closed. */
export const EDIT_MARKER_COMMIT = 0x1a04;
export const INPUT_WHEEL = 0x1103;
/** The screen direction the held movement keys point, sent when it changes: each axis -1, 0 or 1, x to the right and y down. */
export const INPUT_MOVE = 0x1106;
/** Answered later with the surface's handle, 0 refused: a canvas the shell hands over, drawn by the kernel alone from then on. It draws nothing until SCENE_DRAW says what. */
export const SURFACE_OPEN = 0x1501;
/** The canvas's layout box in device pixels, transforms aside; the buffer it is drawn into is the kernel's. */
export const SURFACE_RESIZE = 0x1502;
/** The canvas is going: what it drew and held on the device is let go. */
export const SURFACE_CLOSE = 0x1503;
/** What a surface the shell opened draws, until its SURFACE_CLOSE. The world, with the
 * sensors watched while it shows; or a character alone, as the picker shows it, a second one on the same surface
 * replacing the first. EBADF where the surface is not open, EBUSY where it already draws the other content, EINVAL
 * where the character's fields are not all given for a character, any is given for the world, or no character is
 * named by them. */
export const SCENE_DRAW = 0x1704;
/** A setting set from the sheet or the character picker; refused until the kernel has read the
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
 * the camera, sent by the device app. */
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

// ---- shell, import ----

/** Start watching. */
export const SENSOR_START = 0x1201;
/** Stop watching. */
export const SENSOR_STOP = 0x1203;
/** The text in the platform's own family at the weight and size, broken into lines no
 * wider than the width where it can be; at `at` in the kernel's memory, how many lines,
 * then a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
 * (sys/io/runtime.json LAYOUT_ANSWER, LINE). A call: answered before it returns. */
export const TEXT_LAYOUT = 0x1c01;
/** One line drawn in the platform's own family, starting at x with the middle of its em
 * box on the middle row, into `width` by `height` pixels at `at` in the kernel's memory,
 * RGBA a byte a channel, top row first, alpha not premultiplied. A call: answered before
 * it returns. */
export const TEXT_RASTER = 0x1c02;
/** The UI's snapshot, when it changed: a Snapshot packet far past a ring frame, so a
 * block the kernel holds, valid until the next snapshot. */
export const SHELL_SNAPSHOT = 0x1b01;
/** The view's azimuth and scale, when either moved: a frame of its own, so a rotation or a
 * zoom, which moves them every frame, leaves the snapshot as it was. */
export const SHELL_VIEW = 0x1b02;

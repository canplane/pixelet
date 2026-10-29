# The kernel reads glTF itself, and an upload waits for its ingest

**2026-10-26** — The kernel keeps its own glTF parser, now hardened to refuse a
file it cannot trust rather than crash on it (pxl-render `asset/gltf.rs`). The
libraries were read and none is safe on hostile input as it stands. Opening
uploads waits on:

1. **An ingest.** The server decodes an upload with a library in a process that
   can fail alone, writes a narrow GLB, and runs the kernel's `parse_gltf` on it
   as the last gate.
2. **A bound on the file's size.** The document is checked whole before the
   first pause.
3. **`register_model` sliced.** It lays out every vertex in one poll.
4. **Materials and textures.** The parser reads neither, so an upload's own
   textures are not drawn.

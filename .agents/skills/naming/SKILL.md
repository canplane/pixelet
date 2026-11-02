---
name: naming
description: Recommendations for naming in pixelet — short, plain names read where they stand; the names chosen with the user; names someone else owns; renaming. Use before naming, renaming or reviewing names, in code, spec JSON or a backticked identifier in a document.
---

# Naming

- Let the module path qualify a name: inside a module a short, loose name
  usually does; a name read across many modules does better saying its thing
  exactly.
- Prefer short, plain names, as in UNIX V6 and K&R's C, and the abbreviations
  programmers already use (`idx`, `buf`, `pos`, `px`, `cfg`, `err`, `tmp`;
  `pkt` for a wire packet) over coined ones; keep a domain word mostly whole.
- Name what the thing is. A padding noun or a vague participle (`kept`,
  `held`) says little; a plain word that reads at once (`reach`) is fine.
- Leave a name that reads fine, and don't lengthen it to repeat its module.
- Mind a loose word mostly where several modules use it for different things.
- Snake_case and SCREAMING tend to abbreviate; PascalCase types, camelCase and
  the React shell's own names tend to stay whole (`let cfg: Config`).
- Use a plural for a collection, and a unit suffix on a length (`_u` the
  world's units, `_merc` a meter on the EPSG:3857 plane, `_m` a meter on the
  ground), on the value where a person sees it (`29.0 m`).
- Name a new concept the parts share with the user, and use the names already
  chosen with the user:

  | thing | name | not |
  | --- | --- | --- |
  | where the camera looks from | `eye` | camera position |
  | the point the camera turns about (the eye itself in first person) | `pivot` | focus, center, target |
  | where the pivot is going | its `to` (`pivot_to`, `put_pivot`, `resolve_to`) | target |
  | the direction the camera faces | `facing` | look |
  | the camera modes | `orbit`, `first_person` (the UI says 1인칭/3인칭) | free |
  | what a mouse, touch or controller points with | `pointer` | cursor |
  | the raster's nearest sample at each pixel | the visibility buffer | pick |
  | a frame's number on xpute's rings, its meaning the program's | a `cmd`, a `Command` | vec, vector, opcode |
  | what the shell asks of the kernel | a command | |
  | a guest's call into its host, run now or after the turn | a hostcall | system call, hypercall |
  | the number a frame names at the guest's far side, 0 the host | a port | endpoint, worker id |

  `at` is where a thing is, `to` where it is going.
- Keep WebGPU's words in xpute's `gpu/`.
- Treat a name between the client and a server as protocol, and a name kept in
  storage (a bucket object, a storage key, a baked file's field) as needing a
  version bump to change; a setting's name can change, since storage keeps its
  `number`.
- When renaming, search what the language server doesn't see: the
  `rust/client/shader` crate, generated code, and the `.wgsl` files that read a
  render setting by its path's name.
- Watch a renamed C++ struct field being taken by a same-named parameter of its
  own method (`Section::at` read `off + off` until `this->off`).
- Let a backticked identifier in a comment or document follow the code; leave
  a prose word as it is.

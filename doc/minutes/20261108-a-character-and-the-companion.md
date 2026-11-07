# A character, and the companion

**2026-11-08** — recorded with the user. Main was at `b034c0bf54` when it began
and at `65d8303937` when the work was rebased onto it; the work is the one
commit on `scene` that follows it.

## What was asked

Whether what the scene draws, the actors and the assets, was built as objects.
The scene asked its parts in a different shape each; the person and the pet were
held twice, by the companion and again by each picker preview; one person or pet
had three names (`Actor`, `Character`, the kind `actor`) and two wrappers around
it (`Figure`, `Preview`); and the picker's preview drew nothing, culled by the
world's eye at 100,000 units under the sea.

## What the user decided

- **One word for a person or a pet: character.** "actor란 이름은 박멸하고
  character로 확실히 통일하든가 해." The kind, its catalog, its files and the
  type are the character's; "에셋적으로는 character라는 말이 좋다". The kind's
  number stays 3, so nothing stored changes.
- **The companion is who goes with the player**, the person and the pet the
  player chose, kept by name in the messages: "i18n 메시지 상으로는 난
  companion이라는 말이 좋다". It is the part that keeps them, not a type
  around each.
- **The person is the person.** The drawer's label for it says 사람, Person,
  人物, not 캐릭터.
- **The picker uses the person and the pet as they are**, no type or scene of
  its own: "preview에 character랑 pet 그냥 그대로 갖다쓰면 되는데".
- **Each view sees by its own eye.** Zooming the world out does not empty the
  picker.
- **Parent over child.** No path where a thing holds itself again
  (`person.character`).
- **A refactor reduces**: no new word, no added state, a duplicated
  responsibility merged or removed, each name chosen and its reason written.

## What it became

```
companion.rs  Companion: characters[Group], pivot_floor, pet_floor
                each a Character: its row, its clips and palette,
                its transform, its walk (Walk)
surface.rs    character surfaces: (surface, group, bg); each draws the
              companion's character of the group by its own eye
scene.rs      animate, draw: the parts in order, the surfaces' groups
              handed to the companion
```

- The world draws a character where its walk stands it, by decor's tiers, and
  nothing while the eye is the pivot; the companion stays loaded then, placed
  nowhere.
- A character surface's draw is handed to the renderer whole, held by no
  list: its uniform at slot 0 of the pass's own buffer (pxl-render
  `scaled_draw`, gpu/pass/character.rs). The world's list is the world's alone.
- A registered model is its number; its clip count and scale are the
  registry's `Model`'s.
- `Transform.scale` is the scale over the model as registered, as an import's
  scale is the asset's, so a holder knows no model's own scale.

## The names, and why

| Before | After | Why |
|---|---|---|
| kind `actor`, `actor.json`, `asset/content/actor/` | `character` | The user's word; the UI, the protocol and the kernel's catalog row already said character. |
| `Actor` and `Character` | `Character` | One person or pet, one type. |
| `Figure` | gone; its walk is `Walk`, a character's part | It held the character again (`person.character`). |
| `CompanionProps` | `Camera` | What control hands the walk of the camera: where its pivot goes, the pivot's floor, its heading and eye, whether it is dragged, steered or following. `Pivot` was weighed: the heading and the eye are the camera's. "Props" is React's. |
| `path.rs` | in `companion.rs` | `WalkPlan`, `SettleTracker` and `heading_to` are the walk's parts. |
| `Preview`, the preview's second instance, `CASE_DEPTH_U` | gone; `SURFACES` keeps (surface, group, bg) | The surface draws the companion's character. |
| `ModelHandle` | the model's number | It copied the registry's clip count and scale; "handle" is an upload's. |
| `Transform.rot_z` | `rot` | The abbreviation's word, and the shader's and `Ghost::Decor`'s. |
| `ACTOR_UPLOADS` | `CHARACTER_UPLOADS` | A palette a character, two now. |
| step `preview` | gone | Nothing steps a surface. |
| the picker's preview: surface role `PREVIEW`, `character/preview.rs`, pxl-render `pass/preview.rs` `Preview`, the span `preview`, the snapshot's `previews`, `CASE_*`, the shell's `CharacterPreview` | the character surface: `CHARACTER`, `character/surface.rs`, `pass/character.rs` `CharacterPass`, the span `character`, `character_surfaces`, `CHARACTER_*`, `PickedCharacter` | The protocol already says `SceneContent::{World, Character}`, the shell `draw_world` and `draw_character`, and a world surface is the word beside it: no new word. It shows the equipped character as it is, nothing before a choice. `portrait` was tried and left: a word the code did not need. |
| control's `Pending`, `pending()`, `pending_step()` | `Preview`, `preview()`, `preview_step()` | The edit the pointer would make, the shape `domain/preview.rs` gives both heads; the kernel's "pending" is what is asked and not yet answered. |
| `SceneDraw.number` | gone | The surface shows the companion's character of the group. |

## What was weighed and not taken

- A viewport with a scene of its own for the picker.
- A draw list of the picker's own (`PIPELINE_PREVIEW`): it repeats the skinned
  pipeline's descriptor, and shifts every render pipeline's number.
- `Instance` for the playing part: a new word for what the character is.
- The walk's state laid flat into `Character`: the walk is a part it has.
- A `Scene` struct holding every part: main had made each part keep its own
  state (`f746a20019`), and check-rules refuses a part naming the scene's root.

## The frame test

The kernel's frame test failed from `065a4246b1` on: "the view did not rest
within 10 idle turns". That commit took the bound down to `JITTERS +
FRAME_LATENCY` and the frames before it, while the wheel's zoom eases for some
50 turns, so the idle turns were counted over a camera still moving: it moved
to the 32nd of them, and the last ask came 7 turns after it stopped. The kernel
was right. The harness now lets the zoom's ease end, by the snapshot's
`zooming`, before it counts (`Run::settle`).

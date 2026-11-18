# The record

What the system is to be is stated by `ARCHITECTURE.md`,
`xpute/ARCHITECTURE.md` and the READMEs beside the code, each its part's
contract. The code is what it is now, its debt and its work in progress
among it: being in the code does not make something a rule. This folder holds
how it came to be that way and what is next, the way a company keeps its
agenda, its minutes and its reports beside its rules.

- [backlog.md](./backlog.md) — the agenda: what is queued, in priority order,
  one line each, pointing at where its reasoning lives.
- [upstream.md](./upstream.md) — the defects found in what the project builds
  on, by upstream project, each with the records it rests on and what the tree
  does about it; an entry goes when it is found fixed or found to be ours.
- [minutes/](./minutes/) — one meeting's record a file, `YYYYMMDD-topic.md`:
  the meeting's conclusions, each with whose it is and the reason that decided
  it, and not the turns of the talk that led there. While the meeting lasts the
  file is written whole each time a conclusion changes; a design or a report
  written for a meeting becomes its minutes once the meeting is over.
- [minutes/archive/](./minutes/archive/) — older minutes, moved here on the
  user's word.
- [experiment/](./experiment/) — measurements, dated, each with its procedure,
  its numbers and what produced them; evidence under those conditions, not a
  rule.
- [lesson/](./lesson/) — what was learned the hard way and holds beyond one
  change, a file a lesson or a file a genre gathering a part's lessons with their
  sources: what to keep clear of and how to check it, each
  pointing at the experiment that showed it.
- [gpu/](./gpu/) — the GPU's and WebGPU's records gathered in one place: its
  minutes, its experiments and its lessons, laid out as above.
- [note/](./note/) — what the user recalls afterward of how something came to
  be, no meeting's, a file a subject, `YYYYMMDD-topic.md`, with what the history
  shows beside it: testimony of why, not a decision.

A decision of the user's that changes a contract is carried into the document
that states that contract. The user's explicit word now applies within its scope
before the documents catch up.

Where the code and a stated contract disagree, neither the code nor an old
record settles it alone.

A record is revised only on the user's word, in place and whole, so that it
reads as written right: no note goes beneath it and nothing in it says it was
corrected. What stays is what a reader of the code now needs, what holds and the
reason that decided it; the trail to it goes, a commit's hash, a state passed
through, a name no longer in the tree. The `> Since <date> (<commit>): …` notes
that records hold from before stay as they are, and none is added.

A report or a request from outside the project is a GitHub issue. What is
resolved in one is written here, and the issue links to it.

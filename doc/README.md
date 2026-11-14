# The record

What the system is to be is stated by `ARCHITECTURE.md`,
`xpute/ARCHITECTURE.md` and the READMEs beside the code, each its part's
contract. The code is what it is now, its debt and its work in progress
among it: being in the code does not make something a rule. This folder holds
how it came to be that way and what is next, the way a company keeps its
agenda, its minutes and its reports beside its rules.

- [backlog.md](./backlog.md) — the agenda: what is queued, in priority order,
  one line each, pointing at where its reasoning lives.
- [minutes/](./minutes/) — one meeting's record a file, `YYYYMMDD-topic.md`:
  what was asked, what was weighed, and what was resolved, if anything was. A
  design or a report written for a meeting becomes its minutes once the meeting
  is over.
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

A record is not rewritten. Where one defers something or leaves it open, or
decides something since reversed, and that is found when the record is read, a
note goes beneath it: `> Since <date> (<commit>): …`, saying what holds now.

A report or a request from outside the project is a GitHub issue. What is
resolved in one is written here, and the issue links to it.

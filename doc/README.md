# The record

What the system is lives in the code, `AGENTS.md`, `ARCHITECTURE.md` and
the READMEs beside the code: they are what binds today. This folder holds
how it came to be that way and what is next, the way a company keeps its
agenda, its minutes and its reports beside its rules.

- [backlog.md](./backlog.md) — the agenda: what is queued, in priority
  order, one line each, pointing at where its reasoning lives.
- [minutes/](./minutes/) — one meeting's record a file,
  `YYYYMMDD-topic.md`: what was asked, what was weighed, and what was
  resolved, if anything was. A design or a report written for a meeting
  becomes its minutes once the meeting is over. Minutes are never edited.
- [minutes/archive/](./minutes/archive/) — minutes the user has retired.
  A meeting's minutes retire when what they resolved is carried by the
  code and the rules, or when they are discarded; success and failure
  retire alike. They move here once, unchanged, on the user's word.
- [experiment/](./experiment/) — measurements, dated, each with its
  procedure, its numbers and what produced them.

Minutes are records of their day, not rules. A resolution binds once the
code, `AGENTS.md` or `ARCHITECTURE.md` carries it or cites it; where one
of them and the minutes disagree, they win. Archived minutes are never a
ground for a judgment now, and something at odds with them is no
contradiction. Minutes that replace an earlier resolution say which.

A report or a request from outside the project is a GitHub issue. What is
resolved in one is written here, and the issue links to it.

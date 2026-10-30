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
  is over. What it attributes to the user explicitly is an adopted decision;
  what it proposes, tries or measures is not.
- [minutes/archive/](./minutes/archive/) — older minutes, moved here on the
  user's word.
- [experiment/](./experiment/) — measurements, dated, each with its procedure,
  its numbers and what produced them; evidence under those conditions, not a
  rule.

An adopted decision is checked for its scope, and for whether a later adopted
decision replaced it; minutes that replace an earlier one say which. One that
changes a contract is carried into the document that states that contract, and
its reasoning stays here. The user's explicit word now applies within its scope
before the documents catch up.

Where the code and a stated contract disagree, these records and the history
tell debt, an outdated document and an unfinished transition apart; neither
the code nor an old record settles it alone.

A report or a request from outside the project is a GitHub issue. What is
resolved in one is written here, and the issue links to it.

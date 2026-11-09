// ts/dev/example/asker/command.spec.ts
//
// GENERATED from rust/dev/example/asker.json

// Every command, by the side that sends it.

// ---- asker, export ----

/** Carries two numbers. The core answers with their sum plus the number ASK answers. */
export const SUM = 0x1001;

// ---- asker, import ----

/** Asks the runtime for a number, which the core adds to the sum. */
export const ASK = 0x1002;

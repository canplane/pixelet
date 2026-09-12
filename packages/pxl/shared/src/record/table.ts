// @pxl/shared/record/table.ts

/**
 * The column layer: a record is a table of named columns on the tree
 * packet, and every record kind this project keeps is one of these.
 *
 *   [kind str, version u32, keys [str...], required u8[], count u32, column...]
 *
 * `keys` names the columns in order; `required` has a byte per column, 1
 * where a reader that does not know the column must refuse the record
 * rather than skip it (glTF's `extensionsRequired`, PNG's critical bit).
 * `count` is the row count every column has. A column is a typed-array
 * leaf, or a branch of `str` leaves read one row at a time.
 *
 * What this adds to the packet is self-description: a reader finds a
 * column by name, a writer can add one without the readers that do not
 * care noticing, and a record says what it is. What it does not add is a
 * schema system — a record kind's module says which keys mean what and
 * what type each is
 * (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { BigTypedArray, TypedArray, U8Array } from "@xpute/core/kit/array.ts";
import { type BranchCursor, type BranchView, encoder, type NodeCursor, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

export type ColumnValue = TypedArray | BigTypedArray | readonly string[];

export type Column = {
  key: string;
  value: ColumnValue;
  /** A reader that does not know this column must refuse the record. */
  required?: boolean;
  /** Components per row, for a column that packs a vector per row (an
   * extent is three, a quaternion four). The kind's module knows it; the
   * record does not say it. */
  stride?: u32;
};

function put_column(b: BranchView, value: ColumnValue): void {
  if (Array.isArray(value)) {
    b.branch((s) => {
      for (const v of value as readonly string[]) s.str(v);
    });
    return;
  }
  // The generic lowering maps each typed array to its own leaf kind.
  b.put(value as TypedArray);
}

/** Encodes one record. Every column must have `count` rows. */
export function encode_table(kind: string, version: u32, count: u32, columns: readonly Column[]): U8Array {
  for (const c of columns) {
    const rows = c.value.length / (c.stride ?? 1);
    if (rows !== count) throw new Error(`record ${kind}: column ${c.key} has ${rows} rows, not ${count}`);
  }
  const required = new Uint8Array(columns.length);
  columns.forEach((c, i) => (required[i] = c.required ? 1 : 0));
  return encoder.encode(
    new TreeView().branch((__) => {
      __.str(kind).u32(version);
      __.branch((k) => {
        for (const c of columns) k.str(c.key);
      });
      __.u8_array(required).u32(count);
      for (const c of columns) put_column(__, c.value);
    }),
    { init_cap: 1 << 12 },
  );
}

const HEADER = 5;

/** What a record can be opened from: its own packet, or a branch inside a
 * larger packet a container grafted it into. */
export type RecordSource = U8Array | BranchCursor;

export function record_root(source: RecordSource): BranchCursor {
  return source instanceof Uint8Array ? new TreeReader(source).read_branch() : source;
}

/**
 * A record opened over its packet, or over the branch a container holds
 * it in. Columns are cursors into the packet, read on demand; a typed
 * column is a view, not a copy.
 *
 * `known` is what the reader understands. A required column outside it is
 * refused here, once, rather than misread later.
 */
export class Table {
  readonly kind: string;
  readonly version: u32;
  readonly count: u32;
  readonly keys: readonly string[];
  private readonly root: BranchCursor;
  private readonly index = new Map<string, u32>();

  constructor(source: RecordSource, known: readonly string[]) {
    this.root = record_root(source);
    this.kind = this.root.at(0).get<string>();
    this.version = this.root.at(1).get<u32>();
    this.keys = this.root.at(2).as_branch().get<string[]>();
    const required = this.root.at(3).get<U8Array>();
    this.count = this.root.at(4).get<u32>();
    this.keys.forEach((key, i) => this.index.set(key, i));
    for (let i = 0; i < this.keys.length; i++) {
      if (required[i] && !known.includes(this.keys[i])) {
        throw new Error(`record ${this.kind} v${this.version}: required column ${this.keys[i]} is not understood`);
      }
    }
  }

  has(key: string): boolean {
    return this.index.has(key);
  }

  /** The column's cursor, or undefined when the record does not carry it. */
  column(key: string): NodeCursor | undefined {
    const i = this.index.get(key);
    return i === undefined ? undefined : this.root.at(HEADER + i);
  }

  /** A typed-array column as a view into the packet. Throws when absent:
   * the caller decided it is required. */
  lanes<T extends TypedArray | BigTypedArray>(key: string): T {
    const c = this.column(key);
    if (!c) throw new Error(`record ${this.kind}: no column ${key}`);
    return c.get<T>();
  }

  /** A typed-array column, or null when absent. */
  lanes_opt<T extends TypedArray | BigTypedArray>(key: string): T | null {
    const c = this.column(key);
    return c ? c.get<T>() : null;
  }

  /** A string column, read one row at a time through the branch. */
  strings(key: string): BranchCursor | null {
    const c = this.column(key);
    return c ? c.as_branch() : null;
  }
}

/** One row's string, from a column `strings()` returned. */
export function str_at(column: BranchCursor, row: u32): string {
  return column.at(row).get<string>();
}

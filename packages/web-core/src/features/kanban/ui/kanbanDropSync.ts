export type DropRow = {
  id: string;
  status_id: string;
  sort_order: number;
  updated_at: string;
};
type Drop = {
  id: string;
  status_id: string;
  sort_order: number;
  expected: number;
  crossColumn: boolean;
};
type Pending = Drop & { saved?: DropRow; persisted?: boolean };

/** Serialize writes, while keeping the newest local intent visible until observed. */
export class KanbanDropSync {
  readonly pending = new Map<string, Pending>();
  private tail = Promise.resolve();
  private active = true;

  constructor(
    private save: (drop: Drop) => Promise<void>,
    private read: () => Promise<DropRow[]>,
    private changed: () => void,
    private notify: (kind: 'save' | 'sync' | 'conflict') => void,
    private refresh: () => void
  ) {}

  activate() {
    this.active = true;
  }

  dispose() {
    this.active = false;
  }

  enqueue(drop: Drop) {
    const previous = this.pending.get(drop.id);
    const entry: Pending = { ...drop };
    this.pending.set(drop.id, entry);
    this.changed();
    this.tail = this.tail.then(async () => {
      // Accepted drops still persist if the user leaves the board; disposal
      // only suppresses observations and UI callbacks for the old view.
      // A second drop may precede the first response/snapshot. Use the rank
      // requested by our preceding write. Never adopt a conflicting promotion
      // as our baseline: doing so could overwrite activity the user never saw.
      const expected = previous
        ? previous.persisted
          ? previous.sort_order
          : previous.expected
        : drop.expected;
      entry.expected = expected;
      try {
        await this.save({ ...drop, expected });
        entry.persisted = true;
      } catch {
        if (!this.active) return;
        if (this.pending.get(drop.id) === entry) {
          this.pending.delete(drop.id);
          this.notify('save');
        }
        this.refresh();
        this.changed();
        return;
      }
      if (!this.active) return;
      // A successful HTTP response does not imply that the requested rank won.
      // The API intentionally keeps activity promotions on an order conflict.
      try {
        const rows = await this.read();
        if (!this.active) return;
        entry.saved = rows.find((row) => row.id === drop.id);
        if (
          this.pending.get(drop.id) === entry &&
          (!entry.saved ||
            entry.saved.status_id !== drop.status_id ||
            entry.saved.sort_order !== drop.sort_order)
        ) {
          this.notify('conflict');
        }
        if (!entry.saved && this.pending.get(drop.id) === entry)
          this.pending.delete(drop.id);
        this.refresh();
        this.changed();
      } catch {
        if (this.active) {
          if (this.pending.get(drop.id) === entry) this.notify('sync');
          this.refresh();
        }
      }
    });
    return this.tail;
  }

  async retry() {
    // Only confirm writes that had finished before this read started. A
    // delayed recovery snapshot must never acknowledge a newer in-flight drop.
    const targets = [...this.pending].filter(([, entry]) => entry.persisted);
    try {
      const rows = await this.read();
      if (!this.active) return;
      for (const [id, entry] of targets) {
        if (this.pending.get(id) === entry) {
          entry.saved = rows.find((row) => row.id === entry.id);
          if (!entry.saved) {
            this.pending.delete(id);
            this.notify('conflict');
          } else if (
            entry.saved.sort_order !== entry.sort_order ||
            entry.saved.status_id !== entry.status_id
          ) {
            this.notify('conflict');
          }
        }
      }
      this.refresh();
      this.changed();
    } catch {
      if (this.active) this.notify('sync');
    }
  }

  observe(rows: DropRow[]) {
    for (const [id, entry] of this.pending) {
      const row = rows.find((row) => row.id === id);
      // Activity-only promotions can change rank without changing updated_at.
      // At an equal revision the actual status/rank must therefore also match.
      if (
        entry.saved &&
        row &&
        (Date.parse(row.updated_at) > Date.parse(entry.saved.updated_at) ||
          (Date.parse(row.updated_at) === Date.parse(entry.saved.updated_at) &&
            row.status_id === entry.saved.status_id &&
            row.sort_order === entry.saved.sort_order))
      ) {
        this.pending.delete(id);
      }
    }
    return this.pending.size > 0;
  }
}

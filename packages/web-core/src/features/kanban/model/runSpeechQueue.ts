export type RunObservation = {
  run_id: string;
  attempt: number;
  issue: string;
  payload: { type: string; data: Record<string, unknown> };
};

export type PendingQuestion = {
  run_id: string;
  issue: string;
  reference: string;
};

type Entry = {
  run: string;
  kind: 'start' | 'terminal' | 'waiting' | 'commentary';
  text: string;
  revision: number;
};

type Speaker = {
  speak: (text: string, done: () => void) => void;
  cancel: () => void;
  isPending?: (run: string) => Promise<boolean>;
  isActive?: (run: string) => Promise<boolean>;
};

/** Owns only pending utterances; speech already in progress is never preempted. */
export class RunSpeechQueue {
  private pending: Entry[] = [];
  private speaking = false;
  private enabled = false;
  private generation = 0;
  private started = new Set<string>();
  private ended = new Set<string>();
  private attempts = new Map<string, number>();
  private questions = new Map<string, Set<string>>();
  private candidates = new Map<
    string,
    { id: string; issue: string; content: string }
  >();
  private revisions = new Map<string, number>();

  private revision(run: string, kind: Entry['kind']) {
    return this.revisions.get(JSON.stringify([run, kind])) ?? 0;
  }

  private invalidate(run: string, kind: Entry['kind']) {
    const key = JSON.stringify([run, kind]);
    this.revisions.set(key, this.revision(run, kind) + 1);
    return this.revision(run, kind);
  }

  constructor(private readonly speaker: Speaker) {}

  enable() {
    this.enabled = true;
  }
  disable() {
    this.enabled = false;
    this.generation++;
    this.pending = [];
    this.started.clear();
    this.ended.clear();
    this.attempts.clear();
    this.questions.clear();
    this.candidates.clear();
    this.revisions.clear();
    this.speaking = false;
    this.speaker.cancel();
  }

  accept(
    events: RunObservation[],
    questions: PendingQuestion[],
    baseline = false
  ) {
    if (!this.enabled) return;
    const current = new Map<string, Set<string>>();
    for (const question of questions) {
      if (!current.has(question.run_id))
        current.set(question.run_id, new Set());
      current.get(question.run_id)!.add(question.reference);
    }
    if (baseline) {
      this.candidates.clear();
      this.questions = new Map(
        [...current].map(([run, refs]) => [run, new Set(refs)])
      );
    }
    for (const { run_id: run, attempt, issue, payload } of events) {
      const previousAttempt = this.attempts.get(run);
      if (previousAttempt !== undefined && attempt < previousAttempt) continue;
      if (previousAttempt !== undefined && attempt > previousAttempt) {
        this.started.delete(run);
        this.ended.delete(run);
        this.questions.delete(run);
        this.candidates.delete(run);
        this.invalidate(run, 'start');
        this.invalidate(run, 'commentary');
        this.invalidate(run, 'waiting');
        this.pending = this.pending.filter(
          (entry) => entry.run !== run || entry.kind === 'terminal'
        );
      }
      this.attempts.set(run, attempt);
      const d = payload.data;
      if (payload.type === 'lifecycle_changed') {
        const status = d.status;
        if (
          status === 'starting' &&
          !this.started.has(run) &&
          !this.ended.has(run)
        ) {
          this.started.add(run);
          this.pending.push({
            run,
            kind: 'start',
            revision: this.revision(run, 'start'),
            text: `${issue} の作業を開始しました`,
          });
        } else if (
          typeof status === 'string' &&
          [
            'succeeded',
            'failed',
            'cancelled',
            'crashed',
            'audit_failed',
          ].includes(status) &&
          !this.ended.has(run)
        ) {
          this.ended.add(run);
          this.candidates.delete(run);
          this.invalidate(run, 'start');
          this.invalidate(run, 'commentary');
          this.invalidate(run, 'waiting');
          this.pending = this.pending.filter((e) => e.run !== run);
          const text =
            status === 'succeeded'
              ? '作業が完了しました'
              : status === 'cancelled'
                ? '作業が停止しました'
                : '作業が失敗しました';
          this.pending.push({
            run,
            kind: 'terminal',
            revision: this.revision(run, 'terminal'),
            text: `${issue} の${text}`,
          });
        }
      } else if (
        payload.type === 'input_resolved' ||
        payload.type === 'approval_resolved'
      ) {
        const reference = String(d.input_id ?? d.approval_id ?? '');
        const known = this.questions.get(run);
        known?.delete(reference);
        if (known?.size === 0) {
          this.questions.delete(run);
          this.invalidate(run, 'waiting');
          this.pending = this.pending.filter(
            (entry) => entry.run !== run || entry.kind !== 'waiting'
          );
        }
      } else if (
        ['input_requested', 'approval_requested', 'async_questions'].includes(
          payload.type
        ) &&
        !baseline &&
        !this.ended.has(run)
      ) {
        const references =
          payload.type === 'async_questions' && Array.isArray(d.references)
            ? d.references.filter(
                (ref): ref is string => typeof ref === 'string'
              )
            : [String(d.input_id ?? d.approval_id ?? '')];
        const valid = references.filter((ref) => current.get(run)?.has(ref));
        if (valid.length) {
          const known = this.questions.get(run) ?? new Set<string>();
          if (known.size === 0)
            this.pending.push({
              run,
              kind: 'waiting',
              revision: this.invalidate(run, 'waiting'),
              text: `${issue} は回答を待っています`,
            });
          for (const ref of valid) known.add(ref);
          this.questions.set(run, known);
        }
      } else if (
        payload.type === 'commentary_candidate' &&
        !this.ended.has(run)
      ) {
        const message = d.message as
          | { message_id?: string; content?: string }
          | undefined;
        if (message?.message_id && message.content?.trim()) {
          // A later text block may become the next commentary. Do not start
          // an older pending report while its classification is unresolved.
          this.pending = this.pending.filter(
            (entry) => !(entry.run === run && entry.kind === 'commentary')
          );
          this.invalidate(run, 'commentary');
          this.candidates.set(run, {
            id: message.message_id,
            issue,
            content: message.content,
          });
        }
      } else if (
        payload.type === 'commentary_confirmed' &&
        !this.ended.has(run)
      ) {
        const candidate = this.candidates.get(run);
        if (candidate && candidate.id === d.message_id) {
          this.candidates.delete(run);
          this.pending = this.pending.filter(
            (entry) => !(entry.run === run && entry.kind === 'commentary')
          );
          this.pending.push({
            run,
            kind: 'commentary',
            revision: this.invalidate(run, 'commentary'),
            text: `${candidate.issue}。${candidate.content}`,
          });
        }
      } else if (payload.type === 'message' && !this.ended.has(run)) {
        this.candidates.delete(run);
        const message = d.message as
          | { role?: string; content?: string }
          | undefined;
        if (
          d.final_output === false &&
          message?.role === 'assistant' &&
          message.content?.trim()
        ) {
          this.pending = this.pending.filter(
            (e) => !(e.run === run && e.kind === 'commentary')
          );
          this.pending.push({
            run,
            kind: 'commentary',
            revision: this.invalidate(run, 'commentary'),
            text: `${issue}。${message.content}`,
          });
        }
      }
    }
    for (const [run] of this.questions) {
      if (!current.has(run)) this.invalidate(run, 'waiting');
    }
    this.pending = this.pending.filter(
      (entry) => entry.kind !== 'waiting' || current.has(entry.run)
    );
    for (const [run, refs] of this.questions) {
      for (const ref of refs) if (!current.get(run)?.has(ref)) refs.delete(ref);
      if (!refs.size) this.questions.delete(run);
    }
    this.pump();
  }

  private pump() {
    if (!this.enabled || this.speaking || !this.pending.length) return;
    const priority = (entry: Entry) =>
      entry.kind === 'terminal' || entry.kind === 'waiting'
        ? 0
        : entry.kind === 'start'
          ? 1
          : 2;
    let index = 0;
    for (let i = 1; i < this.pending.length; i++)
      if (priority(this.pending[i]) < priority(this.pending[index])) index = i;
    const [entry] = this.pending.splice(index, 1);
    this.speaking = true;
    const generation = this.generation;
    let finished = false;
    const done = () => {
      if (generation !== this.generation || finished) return;
      finished = true;
      this.speaking = false;
      this.pump();
    };
    const speak = () => {
      if (generation === this.generation && this.enabled)
        this.speaker.speak(entry.text, done);
    };
    const verify =
      entry.kind === 'waiting'
        ? this.speaker.isPending
        : entry.kind !== 'terminal'
          ? this.speaker.isActive
          : undefined;
    if (verify) {
      void verify(entry.run).then((valid) => {
        if (generation !== this.generation) return;
        if (!valid || entry.revision !== this.revision(entry.run, entry.kind)) {
          done();
          return;
        }
        if (this.pending.some((next) => priority(next) < priority(entry))) {
          this.pending.unshift(entry);
          done();
        } else speak();
      }, done);
    } else speak();
  }
}

import { describe, expect, it, vi } from 'vitest';
import { RunSpeechQueue, type RunObservation } from './runSpeechQueue';

const event = (
  run_id: string,
  type: string,
  data: Record<string, unknown> = {}
): RunObservation => ({
  run_id,
  attempt: 1,
  issue: run_id === 'a' ? 'LOCAL-1' : 'LOCAL-2',
  payload: { type, data },
});
const lifecycle = (run: string, status: string) =>
  event(run, 'lifecycle_changed', { status });
const message = (run: string, content: string) =>
  event(run, 'message', {
    final_output: false,
    message: { role: 'assistant', content },
  });

describe('RunSpeechQueue', () => {
  it('serializes two runs, prioritizes urgent events and discards stale work', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([message('a', 'first')], []);
    queue.accept(
      [
        lifecycle('a', 'starting'),
        message('a', 'old'),
        message('a', 'new'),
        lifecycle('b', 'starting'),
        event('b', 'input_requested', { input_id: 'q' }),
      ],
      [{ run_id: 'b', issue: 'LOCAL-2', reference: 'q' }]
    );
    expect(spoken).toEqual(['LOCAL-1。first']);
    done();
    expect(spoken[1]).toContain('LOCAL-2 は回答');
    queue.accept([lifecycle('a', 'succeeded')], []);
    done();
    expect(spoken[2]).toContain('LOCAL-1 の作業が完了');
    done();
    expect(spoken[3]).toContain('LOCAL-2 の作業を開始');
    done();
    expect(spoken).toHaveLength(4);
  });
  it('replaces queued commentary and drops a question answered before playback', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([message('a', 'busy')], []);
    queue.accept(
      [
        message('b', 'old'),
        message('b', 'new'),
        event('b', 'input_requested', { input_id: 'q' }),
      ],
      []
    );
    done();
    expect(spoken).toEqual(['LOCAL-1。busy', 'LOCAL-2。new']);
  });
  it('stops immediately and ignores stale callbacks after off and on', () => {
    const cancel = vi.fn();
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel,
    });
    queue.enable();
    queue.accept([message('a', 'busy'), message('b', 'queued')], []);
    const stale = done;
    queue.disable();
    expect(cancel).toHaveBeenCalledOnce();
    queue.enable();
    queue.accept([message('a', 'fresh')], []);
    stale();
    expect(spoken).toEqual(['LOCAL-1。busy', 'LOCAL-1。fresh']);
  });
  it('does not replay pending questions at subscription and rechecks before speaking', async () => {
    const speak = vi.fn();
    const queue = new RunSpeechQueue({
      speak,
      cancel: vi.fn(),
      isPending: async () => false,
    });
    const question = { run_id: 'a', issue: 'LOCAL-1', reference: 'q' };
    queue.enable();
    queue.accept([], [question], true);
    expect(speak).not.toHaveBeenCalled();
    queue.accept([], []);
    queue.accept(
      [event('a', 'input_requested', { input_id: 'q' })],
      [question]
    );
    await Promise.resolve();
    expect(speak).not.toHaveBeenCalled();
  });
  it('announces one transition while any question for the run remains unanswered', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    const one = { run_id: 'a', issue: 'LOCAL-1', reference: 'one' };
    const two = { run_id: 'a', issue: 'LOCAL-1', reference: 'two' };
    queue.enable();
    queue.accept([], [], true);
    queue.accept(
      [event('a', 'async_questions', { references: ['one', 'two'] })],
      [one, two]
    );
    expect(spoken).toEqual(['LOCAL-1 は回答を待っています']);
    queue.accept([], [two]);
    done();
    expect(spoken).toHaveLength(1);
    queue.accept([], []);
    queue.accept([event('a', 'input_requested', { input_id: 'two' })], [two]);
    expect(spoken).toHaveLength(2);
  });
  it('checks queued commentary for completed runs and honors priority during that check', async () => {
    const spoken: string[] = [];
    let done = () => {};
    let verify = (_value: boolean) => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
      isActive: () =>
        new Promise<boolean>((resolve) => {
          verify = resolve;
        }),
    });
    queue.enable();
    queue.accept([message('a', 'old')], []);
    queue.accept([lifecycle('b', 'succeeded')], []);
    verify(true);
    await Promise.resolve();
    expect(spoken).toEqual(['LOCAL-2 の作業が完了しました']);
    done();
    verify(false);
    await Promise.resolve();
    expect(spoken).toHaveLength(1);
  });
  it('keeps arrival order between questions and terminal events', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([message('a', 'busy')], []);
    queue.accept(
      [
        event('b', 'input_requested', { input_id: 'q' }),
        lifecycle('a', 'failed'),
      ],
      [{ run_id: 'b', issue: 'LOCAL-2', reference: 'q' }]
    );
    done();
    expect(spoken[1]).toBe('LOCAL-2 は回答を待っています');
    done();
    expect(spoken[2]).toBe('LOCAL-1 の作業が失敗しました');
  });
  it('recognizes a new unanswered question after the previous one is answered in the same batch', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept(
      [],
      [{ run_id: 'a', issue: 'LOCAL-1', reference: 'old' }],
      true
    );
    queue.accept(
      [
        event('a', 'input_resolved', { input_id: 'old' }),
        event('a', 'input_requested', { input_id: 'new' }),
      ],
      [{ run_id: 'a', issue: 'LOCAL-1', reference: 'new' }]
    );
    expect(spoken).toEqual(['LOCAL-1 は回答を待っています']);
    done();
  });
  it('discards a start or older commentary superseded while freshness is being checked', async () => {
    const spoken: string[] = [];
    const checks: Array<(valid: boolean) => void> = [];
    const queue = new RunSpeechQueue({
      speak: (text) => {
        spoken.push(text);
      },
      cancel: vi.fn(),
      isActive: () => new Promise<boolean>((resolve) => checks.push(resolve)),
    });
    queue.enable();
    queue.accept([message('a', 'old')], []);
    queue.accept([message('a', 'new')], []);
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual([]);
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual(['LOCAL-1。new']);

    const secondSpoken: string[] = [];
    let check!: (valid: boolean) => void;
    const second = new RunSpeechQueue({
      speak: (text) => {
        secondSpoken.push(text);
      },
      cancel: vi.fn(),
      isActive: () =>
        new Promise<boolean>((resolve) => {
          check = resolve;
        }),
    });
    second.enable();
    second.accept([lifecycle('b', 'starting')], []);
    second.accept([lifecycle('b', 'succeeded')], []);
    check(true);
    await Promise.resolve();
    expect(secondSpoken).toEqual(['LOCAL-2 の作業が完了しました']);
  });
  it('drops an obsolete question during its freshness check even when a new one is pending', async () => {
    const spoken: string[] = [];
    const checks: Array<(valid: boolean) => void> = [];
    const queue = new RunSpeechQueue({
      speak: (text) => {
        spoken.push(text);
      },
      cancel: vi.fn(),
      isPending: () => new Promise<boolean>((resolve) => checks.push(resolve)),
    });
    queue.enable();
    queue.accept(
      [event('a', 'input_requested', { input_id: 'old' })],
      [{ run_id: 'a', issue: 'LOCAL-1', reference: 'old' }]
    );
    queue.accept(
      [
        event('a', 'input_resolved', { input_id: 'old' }),
        event('a', 'input_requested', { input_id: 'new' }),
      ],
      [{ run_id: 'a', issue: 'LOCAL-1', reference: 'new' }]
    );
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual([]);
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual(['LOCAL-1 は回答を待っています']);
  });
  it.each([
    ['succeeded', 'LOCAL-1 の作業が完了しました'],
    ['cancelled', 'LOCAL-1 の作業が停止しました'],
    ['failed', 'LOCAL-1 の作業が失敗しました'],
    ['crashed', 'LOCAL-1 の作業が失敗しました'],
    ['audit_failed', 'LOCAL-1 の作業が失敗しました'],
  ])('announces %s distinctly', (status, expected) => {
    const speak = vi.fn();
    const queue = new RunSpeechQueue({ speak, cancel: vi.fn() });
    queue.enable();
    queue.accept([lifecycle('a', status)], []);
    expect(speak).toHaveBeenCalledWith(expected, expect.any(Function));
  });
  it('does not start overlapping speech when a completion callback fires twice', () => {
    const spoken: string[] = [];
    const callbacks: Array<() => void> = [];
    const queue = new RunSpeechQueue({
      speak: (text, done) => {
        spoken.push(text);
        callbacks.push(done);
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([message('a', 'one')], []);
    queue.accept([message('b', 'two'), lifecycle('a', 'starting')], []);
    callbacks[0]();
    callbacks[0]();
    expect(spoken).toHaveLength(2);
    callbacks[1]();
    expect(spoken).toHaveLength(3);
  });
  it('speaks a Claude candidate only after its own explicit confirmation', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    const candidate = (id: string, content: string) =>
      event('a', 'commentary_candidate', {
        message: { message_id: id, content },
      });
    const confirm = (id: string) =>
      event('a', 'commentary_confirmed', { message_id: id });
    queue.enable();
    queue.accept([candidate('one', 'Checking')], []);
    queue.accept([confirm('other')], []);
    expect(spoken).toEqual([]);
    queue.accept([confirm('one')], []);
    expect(spoken).toEqual(['LOCAL-1。Checking']);
    done();
    queue.accept(
      [
        candidate('final', 'Final answer'),
        lifecycle('a', 'succeeded'),
        confirm('final'),
      ],
      []
    );
    expect(spoken).toEqual([
      'LOCAL-1。Checking',
      'LOCAL-1 の作業が完了しました',
    ]);
  });
  it('forgets unconfirmed Claude candidates on reconnect or off', () => {
    const speak = vi.fn();
    const queue = new RunSpeechQueue({ speak, cancel: vi.fn() });
    const candidate = event('a', 'commentary_candidate', {
      message: { message_id: 'one', content: 'old' },
    });
    const confirm = event('a', 'commentary_confirmed', { message_id: 'one' });
    queue.enable();
    queue.accept([candidate], []);
    queue.accept([], [], true);
    queue.accept([confirm], []);
    queue.accept([candidate], []);
    queue.disable();
    queue.enable();
    queue.accept([confirm], []);
    expect(speak).not.toHaveBeenCalled();
  });
  it('retains only the newest unconfirmed text per run', () => {
    const spoken: string[] = [];
    const queue = new RunSpeechQueue({
      speak: (text) => {
        spoken.push(text);
      },
      cancel: vi.fn(),
    });
    const candidate = (id: string, content: string) =>
      event('a', 'commentary_candidate', {
        message: { message_id: id, content },
      });
    const confirm = (id: string) =>
      event('a', 'commentary_confirmed', { message_id: id });
    queue.enable();
    queue.accept(
      [
        candidate('old', 'old'),
        candidate('new', 'new'),
        confirm('old'),
        confirm('new'),
      ],
      []
    );
    expect(spoken).toEqual(['LOCAL-1。new']);
  });
  it('does not start an older report while a newer Claude text awaits classification', async () => {
    const spoken: string[] = [];
    const checks: Array<(valid: boolean) => void> = [];
    const queue = new RunSpeechQueue({
      speak: (text) => {
        spoken.push(text);
      },
      cancel: vi.fn(),
      isActive: () => new Promise<boolean>((resolve) => checks.push(resolve)),
    });
    queue.enable();
    queue.accept([message('a', 'old')], []);
    queue.accept(
      [
        event('a', 'commentary_candidate', {
          message: { message_id: 'new', content: 'new' },
        }),
      ],
      []
    );
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual([]);
    queue.accept(
      [event('a', 'commentary_confirmed', { message_id: 'new' })],
      []
    );
    checks.shift()!(true);
    await Promise.resolve();
    expect(spoken).toEqual(['LOCAL-1。new']);
  });
  it('announces a retried Run and ignores late events from its previous attempt', () => {
    const spoken: string[] = [];
    let done = () => {};
    const queue = new RunSpeechQueue({
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([lifecycle('a', 'failed')], []);
    queue.accept(
      [
        { ...lifecycle('a', 'starting'), attempt: 2 },
        { ...message('a', 'stale'), attempt: 1 },
      ],
      []
    );
    done();
    expect(spoken[1]).toBe('LOCAL-1 の作業を開始しました');
    queue.accept([{ ...lifecycle('a', 'succeeded'), attempt: 2 }], []);
    done();
    expect(spoken[2]).toBe('LOCAL-1 の作業が完了しました');
    expect(spoken).toHaveLength(3);
  });
});

describe('speech content selection', () => {
  function setup() {
    let all = true;
    const spoken: string[] = [];
    let done = () => {};
    const cancel = vi.fn();
    const queue = new RunSpeechQueue({
      shouldSpeak: (kind) => all || kind === 'terminal' || kind === 'waiting',
      speak: (text, next) => {
        spoken.push(text);
        done = next;
      },
      cancel,
    });
    queue.enable();
    return {
      queue,
      spoken,
      cancel,
      finish: () => done(),
      mode: (value: boolean) => {
        all = value;
      },
    };
  }

  it('all retains starts and progress; results excludes them without replaying old events', () => {
    const s = setup();
    s.queue.accept([lifecycle('a', 'starting'), message('a', 'progress')], []);
    expect(s.spoken[0]).toContain('作業を開始');
    s.finish();
    expect(s.spoken[1]).toContain('progress');
    s.finish();
    s.mode(false);
    s.queue.accept([lifecycle('b', 'starting'), message('b', 'excluded')], []);
    expect(s.spoken).toHaveLength(2);
    s.mode(true);
    s.queue.accept([lifecycle('b', 'starting')], []);
    expect(s.spoken).toHaveLength(2); // Start remains deduplicated; progress was discarded.
    s.queue.accept([message('b', 'new progress')], []);
    expect(s.spoken[2]).toContain('new progress');
  });

  it.each(['succeeded', 'failed', 'cancelled', 'crashed', 'audit_failed'])(
    'results retains terminal %s and waiting, including deduplication',
    (status) => {
      const s = setup();
      s.mode(false);
      const questions = [{ run_id: 'a', issue: 'LOCAL-1', reference: 'q' }];
      s.queue.accept(
        [
          lifecycle('a', 'starting'),
          message('a', 'excluded'),
          event('a', 'input_requested', { input_id: 'q' }),
        ],
        questions
      );
      expect(s.spoken).toEqual(['LOCAL-1 は回答を待っています']);
      s.queue.accept(
        [
          event('a', 'input_requested', { input_id: 'q' }),
          lifecycle('b', status),
        ],
        questions
      );
      s.finish();
      expect(s.spoken[1]).toBe(
        `LOCAL-2 の作業が${status === 'succeeded' ? '完了' : status === 'cancelled' ? '停止' : '失敗'}しました`
      );
      s.finish();
      s.queue.accept([lifecycle('b', status)], questions);
      expect(s.spoken).toHaveLength(2);
    }
  );

  it('switching drops queued starts/reports without interrupting current speech or losing urgent work', () => {
    const s = setup();
    s.queue.accept([message('a', 'in progress')], []);
    const questions = [{ run_id: 'c', issue: 'LOCAL-2', reference: 'q' }];
    s.queue.accept(
      [
        lifecycle('b', 'starting'),
        message('b', 'queued'),
        event('c', 'approval_requested', { approval_id: 'q' }),
        lifecycle('d', 'succeeded'),
      ],
      questions
    );
    s.mode(false);
    expect(s.cancel).not.toHaveBeenCalled();
    expect(s.spoken).toEqual(['LOCAL-1。in progress']);
    s.finish();
    expect(s.spoken[1]).toContain('回答を待っています');
    s.finish();
    expect(s.spoken[2]).toContain('作業が完了');
    s.finish();
    s.mode(true);
    s.queue.accept([], questions);
    expect(s.spoken).toHaveLength(3);
  });

  it('rechecks mode after async validity checks and preserves state of suppressed events', async () => {
    let all = true;
    let resolve!: (valid: boolean) => void;
    const speak = vi.fn();
    const queue = new RunSpeechQueue({
      shouldSpeak: (kind) => all || kind === 'terminal' || kind === 'waiting',
      isActive: () =>
        new Promise<boolean>((done) => {
          resolve = done;
        }),
      speak,
      cancel: vi.fn(),
    });
    queue.enable();
    queue.accept([lifecycle('a', 'starting')], []);
    all = false;
    resolve(true);
    await Promise.resolve();
    expect(speak).not.toHaveBeenCalled();
    queue.accept([lifecycle('a', 'succeeded')], []);
    expect(speak.mock.calls[0][0]).toBe('LOCAL-1 の作業が完了しました');
    speak.mock.calls[0][1]();
    all = true;
    queue.accept([lifecycle('a', 'starting'), message('a', 'old')], []);
    expect(speak).toHaveBeenCalledTimes(1);
  });
});

import { SettingsCheckbox } from './SettingsComponents';

export function OsNotificationEvents({
  mask,
  onChange,
}: {
  mask: number;
  onChange: (mask: number) => void;
}) {
  return (
    <div className="space-y-base">
      <p className="text-sm text-low">
        OS通知はサーバーが動く端末に表示され、設定はこのLVKサーバーの全利用者・全Projectで共通です。上のWeb
        Push登録とは独立しています。変更は画面の保存操作で反映されます。全イベントOFFでも質問・承認の操作は残ります。通知音は従来の設定に従います。
      </p>
      {[
        {
          bit: 1,
          label: '仕様質問待ち',
          description: '実行中のエージェントが回答を求めたとき。',
        },
        {
          bit: 2,
          label: '実行承認待ち',
          description: '実行中のエージェントの操作が承認待ちになったとき。',
        },
        {
          bit: 4,
          label: '実行成功',
          description:
            'Workspaceの実行処理が正常に終了したとき。Run成功・テスト合格・自動マージ公開を保証する通知ではありません。',
        },
        {
          bit: 8,
          label: '実行失敗',
          description:
            'Workspaceの実行処理が失敗したとき。ユーザーによる停止は通知しません。',
        },
      ].map((event) => (
        <SettingsCheckbox
          key={event.bit}
          id={`os-push-event-${event.bit}`}
          label={event.label}
          description={event.description}
          checked={(mask & event.bit) !== 0}
          onChange={(checked) =>
            onChange(checked ? mask | event.bit : mask & ~event.bit)
          }
        />
      ))}
    </div>
  );
}

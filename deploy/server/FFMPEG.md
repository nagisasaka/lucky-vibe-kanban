# FFmpeg 同梱の検証・配布確認

## 実装と現在の証拠

2026-11-25: Dockerfile の `server-tools` の既存 `apt-get install
--no-install-recommends` に Debian bookworm の `ffmpeg` を追加する。
`server-browser` → 公開用 `server` に継承され、最終 user は既存の
`appuser` (10001:10001)。`runtime-base` / `runtime` と dev バンドルは変更しない。
独自ビルド、Remotion、VOICEVOX、LVK の動画 API/UI は追加しない。

この Workspace には Docker CLI と `/var/run/docker.sock` がない。
**候補イメージの実行確認、依存の確定、追加前後の容量測定は未実施**。
ホストの ffmpeg や apt の見積もりを候補の測定結果に代用しない。
容量増分は未測定（0 bytes という意味ではない）。

開始 SHA: `632f24f59e5e86e7b4170ca648350b5a832733c6`。
並行 discovery では SQLite 作業 Workspace が実行中。関連 Manifest の
`20d3160c6c991f79e8ef83f08a952c26358bdc04` は古い COPY の削除、
`906342c79c79e95177de2e692941964e3e5cb8ad` は nano 追加であり、
どちらもイメージ確認の証拠ではない。LOCAL-495 の実イメージ ID はここでは未取得。

## ローカル検証結果

- `pnpm install --frozen-lockfile --ignore-scripts`: 成功。lockfile 差分なし。
  初回 `server:check` の yaml 未導入エラーを解消した。
- `pnpm run server:check`: 34 passed / 1 failed / 8 skipped。
  失敗は `release.test.mjs` の `shell examples parse` が既存の
  `openwiki/operations/server-container.md` に bash fenced block を要求するもの。
  テストと参照 Wiki は開始 SHA と同一で、この変更では修正しない。
  Docker Compose 1 件、nginx 7 件は実行環境不足で skipped。
- Dockerfile を開始時と完全比較し、差分が既存 apt 行への ffmpeg のみであること、
  server への継承と既定 appuser の維持を確認: 成功。
- `sh -n deploy/server/ffmpeg-smoke.sh`、本書の shell block の `bash -n`、
  `git diff --check`: 成功。smoke の実行成功を意味しない。

## VPS での隔離検証（未実行の手順）

1. LOCAL-495 の既存イメージの ID / digest / tag / `.Size` / platform と
   元の build args を記録して保持する。tag の上書き、削除、prune はしない。
   dev ビルド等の実 process と既存 builder の使用状態、空き容量・メモリを
   調べ、重いビルドを重ねない時間帯で順番に行う。既存 builder/cache を利用し、
   cache の削除や設定変更はしない。
2. 同じ checkout/context、`linux/amd64`、builder、build args、外部ツール版、
   全 FROM image digest、apt repository snapshot（Debian security を含む）、
   npm 解決結果を固定して before/after を作る。候補 Dockerfile の `ffmpeg`
   だけを除いたものを before とする。実験用 Dockerfile は task 用一時領域に
   作り、正規 Dockerfile のベース指定や apt 経路は変更しない。
   FROM digest と apt snapshot 固定は両方の実験用 Dockerfile に同じ内容で行う。
   同時刻の連続ビルドだけで同条件と見なさない。パッケージ一覧の差で検証する。
3. `bundled-source` は公開 Git の完全 SHA を取得するため、未 push の SHA を
   `LVK_BUILD_REVISION` に渡すと失敗する。今回の Dockerfile/doc/script 以外の
   context が一致する公開 SHA を確認できる場合のみ、それを両方に使い、
   実験 patch と元 SHA を別途記録する。公開 SHA が得られなければ停止して
   前提不足を報告する。この検証のために push したり source 検証を無効化しない。
4. 既存 builder 名を `BUILDER`、一致する公開 SHA を `REVISION`、実験用
   Dockerfile を `BEFORE_FILE` / `AFTER_FILE` に設定する。新規の専用 tag を
   `BEFORE_TAG` / `AFTER_TAG` に設定し、既存 tag と重複していないことを確認する。
   必要な既存 build args/cache-from は両方に同じ値で付ける。

   ```sh
   docker buildx build --builder "$BUILDER" --platform linux/amd64 \
     --target server --load --build-arg LVK_BUILD_REVISION="$REVISION" \
     --build-arg LVK_BUILD_KIND=development --build-arg LVK_BUILD_DIRTY=true \
     -f "$BEFORE_FILE" -t "$BEFORE_TAG" .
   docker buildx build --builder "$BUILDER" --platform linux/amd64 \
     --target server --load --build-arg LVK_BUILD_REVISION="$REVISION" \
     --build-arg LVK_BUILD_KIND=development --build-arg LVK_BUILD_DIRTY=true \
     -f "$AFTER_FILE" -t "$AFTER_TAG" .
   BEFORE=$(docker image inspect --format '{{.Id}}' "$BEFORE_TAG")
   AFTER=$(docker image inspect --format '{{.Id}}' "$AFTER_TAG")
   docker image inspect --format '{{.Id}} {{.Os}}/{{.Architecture}} {{.Size}} {{.Config.User}}' "$BEFORE" "$AFTER"
   ```

5. `.Size` の after − before を bytes と MiB（bytes / 1048576）で記録する。
   これはローカル非圧縮イメージの論理サイズで、共有 layer を除く実ディスク使用量や
   GHCR 転送サイズではない。LOCAL-495 とも比較するが、他の差があれば
   FFmpeg 単独増分とは区別する。apt の Installed-Size 合計だけで代用しない。
6. 以下を実行する。`--user` を上書きせず、イメージの既定 user を確認する。
   サーバーを起動せず、network/host mount/永続 volume/公開 port を使用しない。

   ```sh
   docker run --rm -i --network none --read-only \
     --tmpfs /tmp:rw,nosuid,nodev,size=64m --cap-drop ALL \
     --security-opt no-new-privileges --entrypoint /bin/sh "$AFTER" \
     -s < deploy/server/ffmpeg-smoke.sh
   ```

   成功条件: appuser UID 10001、ffmpeg/ffprobe 実行、人工 2 秒 FFV1 素材生成、
   途中 1 秒を MPEG-4/MP4 へ変換、ffprobe で形式・寸法・長さ・サイズ確認、
   出力全体の再デコードと SHA-256 読取がすべて exit 0。
   全ログ、image ID、構成オプション・版を保存する。これは全 codec / GPU / 音声 /
   NewsRoom ワークフローの検証ではない。
7. 同じ `docker run --rm --network none --entrypoint ...` で両イメージの
   パッケージ一覧を保存して比較する:

   ```sh
   for side in before after; do
     if [ "$side" = before ]; then img=$BEFORE; else img=$AFTER; fi
     docker run --rm --network none --entrypoint dpkg-query "$img" \
       -W '-f=${binary:Package}\t${Version}\t${Architecture}\t${source:Package}\t${source:Version}\n' \
       > "$side-packages.tsv"
   done
   diff -u before-packages.tsv after-packages.tsv
   ```

   diff の exit 1 は差がある意味。追加だけでなく upgrade/remove も確認し、
   FFmpeg と無関係な差があれば条件を直して再測定する。既に browser 用に存在する
   依存もあるため、Web の依存一覧をそのまま「追加一覧」としない。
8. 下記配布確認を完了し結果を候補 ID に結び付ける。失敗時はログと理由を残し、
   稼働環境へ適用しない。task 専用一時素材だけ片付け、LOCAL-495・共有 cache・
   使用中 image は保持する。main merge、再作成、push、GHCR 公開はこの作業の範囲外。

## 一次資料による配布条件の確認

確認日: 2026-11-25。以下は確認対象と作業項目であり法的結論ではない。
候補未ビルドのため、インストールされる版・全依存に対する確認完了を意味しない。

- [Debian bookworm ffmpeg](https://packages.debian.org/bookworm/ffmpeg) は
  ffmpeg/ffprobe を同一パッケージに収録し、libavcodec59、libavdevice59、
  libavfilter8、libavformat59、libavutil57、libpostproc56、libswresample4、
  libswscale6、libc6、libsdl2-2.0-0 を直接依存として列挙する。
- [Debian FFmpeg copyright](https://metadata.ftp-master.debian.org/changelogs/main/f/ffmpeg/ffmpeg_5.1.9-0%2Bdeb12u1_copyright)
  は標準 Debian バイナリを GPL-2+、extra flavor を GPL-3+ と説明する。
  Debian は non-free ライブラリを組み合わせないとも説明している。
  「FFmpeg は LGPL だけ」と表示しない。候補の `ffmpeg -L` / `-buildconf` と
  実際の copyright を照合する。
- [FFmpeg upstream](https://www.ffmpeg.org/legal.html) も optional GPL 部分と
  外部ライブラリにより条件が変わると説明する。掲載の LGPL linking checklist を
  この CLI 同梱へそのまま当てはめない。特許についても扱っており、対象地域・用途での
  特許上の判断はこの作業では行わない。
- 依存の一次資料例:
  [x264](https://metadata.ftp-master.debian.org/changelogs/main/x/x264/stable_copyright)、
  [x265](https://metadata.ftp-master.debian.org/changelogs/main/x/x265/stable_copyright)
  には GPL-2+ など、
  [LAME](https://metadata.ftp-master.debian.org/changelogs/main/l/lame/stable_copyright)
  には LGPL-2+ / LGPL-2.1+ など、
  [SDL2](https://metadata.ftp-master.debian.org/changelogs/main/libs/libsdl2/stable_copyright)
  には zlib/libpng、BSD、Expat、Apache 等のファイル別記載がある。
  これら stable URL は可変かつ現在の bookworm と同一とは限らず、代表例の調査に限る。
  ライセンス名の出現は全バイナリへの適用を意味しない。
  **追加・更新された各パッケージの実版で、ファイル範囲と適用条件を再確認する必要がある。**
- [Debian Policy 12.5](https://www.debian.org/doc/debian-policy/ch-docs.html#copyright-information)
  に従う `/usr/share/doc/<package>/copyright` と、その参照先
  `/usr/share/common-licenses/` を保持する。現 Dockerfile は apt lists の削除のみで
  これらを削除しない。最終 image で symlink の先も読めることを確認する。
- [GPL v2 §1–3](https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt) は
  著作権・許諾・免責の表示とライセンス写し、バイナリ配布時の対応ソース提供の選択肢を定める。
  [LGPL v2.1 §4](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt) は
  ライブラリのバイナリ配布と対応ソースを扱う。適用する版の全文を各パッケージと照合する。

## 公開前に必要な対応・残課題

- before/after の追加・更新一覧から source package/version へ対応付け、推移的依存も
  含めて copyright 全文と参照 license を採取する。既存依存でも FFmpeg から利用する
  ものは配布確認対象に含める。許諾・免責・NOTICE 等の個別条件を一覧化し、実版の
  Debian copyright / source package を一次資料として記録する。全依存監査は未完了。
- GPL/LGPL 対象について、同じ版の `.dsc`、upstream tarball、Debian patches / build
  scripts を含む source archive を取得・ハッシュ検証して保持し、利用者がバイナリに
  対応するソースを得られる提供経路を決める。別の調査用環境で、候補と同じ Debian
  source repository の `apt-get source --download-only SOURCE=VERSION` 等を用いる。
  稼働コンテナーに deb-src やパッケージを追加しない。
- 対応ソースの同等アクセスでの提供を実装案とするが、保存先・公開リンク・維持責任は
  未確定。Debian/upstream の一般 URL を載せただけで配布義務を満たすとは判定しない。
  `/opt/lvk/source` は LVK のソースであり FFmpeg/依存の対応ソースの代替ではない。
- 公開物に第三者パッケージの著作権・ライセンス・対応ソース案内を含める。
  CI の既存 `org.opencontainers.image.licenses=Apache-2.0` は FFmpeg や全 OS
  パッケージの許諾を表すものとして扱わない。第三者表示の掲載場所とメタデータの
  表現は公開前に確認する。この変更では公開 workflow を変更しない。
- 最終報告に before/after/LOCAL-495 の ID・サイズ・差分、同条件の根拠、appuser smoke
  結果、実パッケージ一覧・表示・対応ソースの保存先、未解決事項を記載する。
  現段階は実装と後続手順の準備までで、配布条件の充足や公開可能性の判定ではない。

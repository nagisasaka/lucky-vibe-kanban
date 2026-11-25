#!/bin/sh
# Run inside the candidate image as its default appuser; no host mounts needed.
set -eu
test "$(id -un)" = appuser
test "$(id -u)" = 10001
ffmpeg -version
ffprobe -version
ffmpeg -buildconf
ffmpeg -L
work=$(mktemp -d /tmp/lvk-ffmpeg.XXXXXX)
trap 'rm -rf "$work"' EXIT HUP INT TERM
ffmpeg -nostdin -hide_banner -loglevel error -f lavfi \
  -i 'testsrc2=size=160x120:rate=10' -t 2 -an -c:v ffv1 "$work/input.mkv"
ffmpeg -nostdin -hide_banner -loglevel error -ss 0.5 -i "$work/input.mkv" \
  -t 1 -an -c:v mpeg4 -pix_fmt yuv420p "$work/output.mp4"
test -r "$work/output.mp4"
test -s "$work/output.mp4"
ffprobe -v error -show_streams -show_format -of json "$work/output.mp4" > "$work/probe.json"
python3 - "$work/probe.json" <<'PY'
import json, sys
with open(sys.argv[1]) as stream:
    result = json.load(stream)
video = [s for s in result['streams'] if s['codec_type'] == 'video']
assert len(video) == 1, result
assert video[0]['codec_name'] == 'mpeg4', result
assert (video[0]['width'], video[0]['height']) == (160, 120), result
assert 0.9 <= float(result['format']['duration']) <= 1.1, result
assert int(result['format']['size']) > 0, result
print(json.dumps(result, indent=2))
PY
ffmpeg -nostdin -hide_banner -loglevel error -i "$work/output.mp4" -f null -
sha256sum "$work/output.mp4"
printf '%s\n' 'FFmpeg non-root conversion/readback passed'

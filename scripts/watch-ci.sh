#!/usr/bin/env bash
# 持续监视指定 CI run 的阶段变化，变化时带时间戳打印（供人工/助手盯进度）。
set -u
RUN="${1:?usage: watch-ci.sh <run-id> [interval]}"
INTERVAL="${2:-30}"
REPO="onepve/SoundPilot"
prev=""
deadline=$(( $(date +%s) + 1500 ))

while [ "$(date +%s)" -lt "$deadline" ]; do
  info=$(gh run view "$RUN" --repo "$REPO" \
    --json status,conclusion,jobs \
    --jq '.status + "|" + (.conclusion // "-") + "|" + ([.jobs[].steps[] | select(.status != "pending") | .name + ":" + (.conclusion // .status)] | join(", "))' 2>/dev/null)
  if [ -n "$info" ] && [ "$info" != "$prev" ]; then
    echo "[$(date +%H:%M:%S)] $info"
    prev="$info"
  fi
  st=$(gh run view "$RUN" --repo "$REPO" --json status --jq .status 2>/dev/null)
  if [ "$st" = "completed" ]; then
    echo "[$(date +%H:%M:%S)] FINAL $(gh run view "$RUN" --repo "$REPO" --json status,conclusion --jq '.status + " " + .conclusion')"
    break
  fi
  sleep "$INTERVAL"
done

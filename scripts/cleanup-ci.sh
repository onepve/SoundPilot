#!/usr/bin/env bash
# 清理 GitHub 仓库历史旧 CI 记录（产物 / 缓存 / 运行记录），保留当前进行中的 run。
# 用法: cleanup-ci.sh <owner/repo> [keep_run_id]
set -uo pipefail
REPO="${1:?usage: cleanup-ci.sh <owner/repo> [keep_run_id]}"
KEEP="${2:-}"

if [ -z "$KEEP" ]; then
  KEEP=$(gh run list --repo "$REPO" --limit 20 --json databaseId,status \
    --jq '[.[] | select(.status != "completed")][0].databaseId // empty')
fi
echo "仓库=$REPO 保留运行=$KEEP"

echo "--- 1) 构建产物 ---"
gh api "repos/$REPO/actions/artifacts?per_page=100" --paginate \
  --jq ".artifacts[] | select(.workflow_run.id != ($KEEP | tonumber)) | \"\(.id) \(.name) \(.size_in_bytes)B\"" |
  while read -r id name size; do
    [ -z "$id" ] && continue
    if gh api -X DELETE "repos/$REPO/actions/artifacts/$id" >/dev/null 2>&1; then
      echo "  已删除产物 $id ($name $size)"
    else
      echo "  删除失败 $id"
    fi
  done

echo "--- 2) Actions 缓存 ---"
gh api "repos/$REPO/actions/caches?per_page=100" --paginate --jq '.actions_caches[] | "\(.id) \(.key)"' |
  while read -r id key; do
    [ -z "$id" ] && continue
    if gh api -X DELETE "repos/$REPO/actions/caches/$id" >/dev/null 2>&1; then
      echo "  已删除缓存 $id ($key)"
    else
      echo "  删除失败 $id"
    fi
  done

echo "--- 3) 历史运行记录 ---"
gh run list --repo "$REPO" --limit 100 --json databaseId,status,conclusion \
  --jq ".[] | select(.databaseId != ($KEEP | tonumber)) | \"\(.databaseId) \(.status) \(.conclusion // \"-\")\"" |
  while read -r id status concl; do
    [ -z "$id" ] && continue
    if gh api -X DELETE "repos/$REPO/actions/runs/$id" >/dev/null 2>&1; then
      echo "  已删除运行 $id ($status $concl)"
    else
      echo "  删除失败 $id（可能为进行中或受保护）"
    fi
  done

echo "--- 清理后剩余 ---"
printf '  产物: %s\n' "$(gh api "repos/$REPO/actions/artifacts" --jq '.total_count')"
printf '  缓存: %s\n' "$(gh api "repos/$REPO/actions/caches" --jq '.total_count')"
printf '  运行记录: %s\n' "$(gh run list --repo "$REPO" --limit 100 --json databaseId --jq 'length')"

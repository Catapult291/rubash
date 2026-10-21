#!/bin/bash
# wsl-bash530.sh — 固定的 WSL GNU Bash 5.3.x baseline 调用入口
#
# 解决两个反复出现的坑：
#   1. Git Bash 的 MSYS 路径转换会把 /mnt/d/... 和 --flags 弄坏
#   2. 测试文件可能有 CRLF，WSL bash 遇到 \r 报 $'\r' 语法错误
#
# 用法：
#   scripts/wsl-bash530.sh <script.sh>          # 运行脚本（原样）
#   scripts/wsl-bash530.sh --strip <script.sh>  # 先去 CRLF 再运行（原目录旁临时副本）
#   scripts/wsl-bash530.sh -c '<command>'        # 运行内联命令（简单场景）
#
# 底层固定调用 /usr/local/bin/bash（5.3.x，跟随 vendored patchlevel 重建，
# 当前 Bash-5.3 patch 20 / rubash#895），不依赖 PATH 顺序。

set -euo pipefail

BASH530="/usr/local/bin/bash"

strip_crlf=false
if [[ "${1:-}" == "--strip" ]]; then
  strip_crlf=true
  shift
fi

if [[ $# -eq 0 ]]; then
  echo "usage: $0 [--strip] <script.sh> | -c '<command>'" >&2
  exit 2
fi

if [[ "$1" == "-c" ]]; then
  shift
  MSYS_NO_PATHCONV=1 wsl "$BASH530" -c "$*"
  exit $?
fi

script="$1"
shift
if [[ ! -f "$script" ]]; then
  echo "error: file not found: $script" >&2
  exit 2
fi

abs_script="$(cd "$(dirname "$script")" && pwd)/$(basename "$script")"

if $strip_crlf; then
  # Git Bash 的 grep 在文本模式下会吞 \r，无法用 grep 检测 CRLF。
  # tr -d '\r' 是幂等的，总是 strip 最简单可靠。
  # 创建临时目录保留原始文件名，这样 ${THIS_SH} ./xxx.sub 相对路径可用。
  src_dir="$(dirname "$abs_script")"
  tmp_dir="$src_dir/.wsl-bash530-strip-tmp"
  rm -rf "$tmp_dir"
  mkdir -p "$tmp_dir"
  tr -d '\r' < "$abs_script" > "$tmp_dir/$(basename "$script")"
  # 同目录下所有 .sub 也 strip（GNU 测试用 ${THIS_SH} ./xxx.sub 引用）
  for sub in "$src_dir"/*.sub; do
    [[ -f "$sub" ]] || continue
    tr -d '\r' < "$sub" > "$tmp_dir/$(basename "$sub")"
  done
  cleanup() {
    rm -rf "$tmp_dir" 2>/dev/null || true
  }
  trap cleanup EXIT
  abs_script="$tmp_dir/$(basename "$script")"
fi

# 转换 Windows 路径到 WSL 路径
win_path=$(cygpath -w "$abs_script" | tr '\\' '/')
wsl_path=$(MSYS_NO_PATHCONV=1 wsl wslpath "$win_path" 2>/dev/null || echo "$abs_script")
wsl_dir=$(dirname "$wsl_path")

# 先 cd 到脚本所在目录再运行，这样 ./xxx.sub 相对路径可用
# 用 bash -c 包装，避免 wsl --cd 的路径转换问题
if [[ $# -gt 0 ]]; then
  MSYS_NO_PATHCONV=1 wsl "$BASH530" -c "cd '$wsl_dir' && exec '$BASH530' '$wsl_path' \"\$@\"" bash "$@"
else
  MSYS_NO_PATHCONV=1 wsl "$BASH530" -c "cd '$wsl_dir' && exec '$BASH530' '$wsl_path'"
fi

#!/usr/bin/env bash
#
# verify-and-install.sh — Run workspace checks via RTK, then install devcli + pm-daemon.
#
# Usage:
#   ./scripts/verify-and-install.sh              # test + clippy + install
#   ./scripts/verify-and-install.sh --test-only  # checks only
#   ./scripts/verify-and-install.sh --install-only
#   ./scripts/verify-and-install.sh --skip-clippy
#   ./scripts/verify-and-install.sh -v           # verbose RTK output
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

RUN_TESTS=true
RUN_CLIPPY=true
RUN_INSTALL=true
RTK_VERBOSE=()

# ── Colors (disabled when not a TTY) ──────────────────────────────────────────

if [[ -t 1 ]]; then
  BOLD=$'\033[1m'
  DIM=$'\033[2m'
  RESET=$'\033[0m'
  CYAN=$'\033[36m'
  GREEN=$'\033[32m'
  RED=$'\033[31m'
  YELLOW=$'\033[33m'
  MAGENTA=$'\033[35m'
  BLUE=$'\033[34m'
  WHITE=$'\033[97m'
  BG_BLUE=$'\033[44m'
  BG_GREEN=$'\033[42m'
  BG_RED=$'\033[41m'
else
  BOLD='' DIM='' RESET='' CYAN='' GREEN='' RED='' YELLOW='' MAGENTA='' BLUE='' WHITE=''
  BG_BLUE='' BG_GREEN='' BG_RED=''
fi

STEP_TOTAL=0
declare -a STEP_NAMES=()
declare -a STEP_STATUS=()
declare -a STEP_DETAIL=()
declare -a STEP_DURATION=()

usage() {
  sed -n '3,12p' "$0" | sed 's/^# \?//'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage ;;
    -v|--verbose) RTK_VERBOSE=(-vv); shift ;;
    --test-only) RUN_INSTALL=false; shift ;;
    --install-only) RUN_TESTS=false; RUN_CLIPPY=false; shift ;;
    --skip-clippy) RUN_CLIPPY=false; shift ;;
    --skip-install) RUN_INSTALL=false; shift ;;
    *) echo "Unknown option: $1" >&2; usage ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo -e "${RED}✗${RESET} Required command not found: ${BOLD}$1${RESET}" >&2
    exit 1
  fi
}

print_banner() {
  local width=62
  local title="devcli · verify & install"
  local subtitle="RTK-powered workspace checks + local install"
  local pad_title=$(( (width - ${#title} - 2) / 2 ))

  echo
  echo -e "${CYAN}╔$(printf '═%.0s' $(seq 1 "$width"))╗${RESET}"
  printf "${CYAN}║${RESET}%*s${BOLD}${WHITE}%s${RESET}%*s${CYAN}║${RESET}\n" \
    "$pad_title" "" "$title" "$(( width - pad_title - ${#title} ))" ""
  local pad_sub=$(( (width - ${#subtitle} - 2) / 2 ))
  printf "${CYAN}║${RESET}%*s${DIM}%s${RESET}%*s${CYAN}║${RESET}\n" \
    "$pad_sub" "" "$subtitle" "$(( width - pad_sub - ${#subtitle} ))" ""
  echo -e "${CYAN}╚$(printf '═%.0s' $(seq 1 "$width"))╝${RESET}"
  echo
}

print_context() {
  echo -e "${DIM}repo${RESET}   ${BLUE}${REPO_ROOT}${RESET}"
  echo -e "${DIM}rtk${RESET}    $(rtk --version 2>/dev/null | head -1 || echo "rtk")"
  echo -e "${DIM}rust${RESET}   $(rustc --version 2>/dev/null || echo "unknown")"
  echo
}

step_begin() {
  local idx="$1"
  local title="$2"
  local cmd="$3"
  echo -e "${MAGENTA}┌─${RESET} ${BOLD}Step ${idx}/${STEP_TOTAL}${RESET} ${DIM}·${RESET} ${BOLD}${title}${RESET}"
  echo -e "${MAGENTA}│${RESET}  ${DIM}\$${RESET} ${cmd}"
}

step_end() {
  local idx="$1"
  local status="$2"
  local detail="$3"
  local duration="$4"
  local icon color bg

  if [[ "$status" == "ok" ]]; then
    icon="✓"; color="$GREEN"; bg="$BG_GREEN"
  else
    icon="✗"; color="$RED"; bg="$BG_RED"
  fi

  echo -e "${MAGENTA}└─${RESET} ${color}${icon}${RESET} ${detail} ${DIM}(${duration}s)${RESET}"
  echo

  STEP_NAMES+=("$idx")
  STEP_STATUS+=("$status")
  STEP_DETAIL+=("$detail")
  STEP_DURATION+=("$duration")
}

run_rtk_step() {
  local idx="$1"
  local title="$2"
  shift 2
  local -a cmd=("$@")
  local cmd_display
  cmd_display=$(printf '%q ' "${cmd[@]}")

  step_begin "$idx" "$title" "$cmd_display"

  local log
  log="$(mktemp "${TMPDIR:-/tmp}/devcli-verify.XXXXXX")"
  local start end elapsed status detail

  start=$(date +%s)
  set +e
  if ( cd "$REPO_ROOT" && "${cmd[@]}" ) 2>&1 | tee "$log"; then
    status="ok"
  else
    status="fail"
  fi
  set -e
  end=$(date +%s)
  elapsed=$(( end - start ))

  detail="$(extract_summary "$log" "$status")"
  step_end "$idx" "$status" "$detail" "$elapsed"

  rm -f "$log"
  [[ "$status" == "ok" ]]
}

extract_summary() {
  local log="$1"
  local status="$2"
  local line

  if [[ "$status" != "ok" ]]; then
    line="$(grep -E '(^error|^FAILED|test result: FAILED|failed to compile)' "$log" | tail -1 || true)"
    if [[ -n "$line" ]]; then
      echo "$line" | sed 's/^[[:space:]]*//' | cut -c1-80
      return
    fi
    echo "failed — see output above"
    return
  fi

  line="$(grep -E '^cargo test:' "$log" | tail -1 || true)"
  [[ -n "$line" ]] && { echo "$line"; return; }

  line="$(grep -E '^cargo clippy:' "$log" | tail -1 || true)"
  [[ -n "$line" ]] && { echo "$line"; return; }

  line="$(grep -E '^cargo install \(' "$log" | tail -1 || true)"
  if [[ -n "$line" ]]; then
    local pkg
    pkg="$(sed -n 's/^cargo install (\([^ ]*\).*/\1/p' <<<"$line")"
    echo "installed ${pkg}"
    return
  fi

  line="$(grep -oE 'executable `[^`]+`' "$log" | tail -1 | tr -d '`' || true)"
  [[ -n "$line" ]] && { echo "installed ${line#executable }"; return; }

  echo "completed successfully"
}

count_planned_steps() {
  local n=0
  $RUN_TESTS && n=$((n + 1))
  $RUN_CLIPPY && n=$((n + 1))
  $RUN_INSTALL && n=$((n + 2))
  echo "$n"
}

print_summary() {
  local width=62
  local all_ok=true
  local i

  echo -e "${CYAN}╔$(printf '═%.0s' $(seq 1 "$width"))╗${RESET}"
  printf "${CYAN}║${RESET}  ${BOLD}${WHITE}Summary${RESET}%*s${CYAN}║${RESET}\n" $(( width - 9 )) ""
  echo -e "${CYAN}╠$(printf '═%.0s' $(seq 1 "$width"))╣${RESET}"

  for i in "${!STEP_NAMES[@]}"; do
    local label="${STEP_DETAIL[$i]}"
    local dur="${STEP_DURATION[$i]}"
    local mark color

    if [[ "${STEP_STATUS[$i]}" == "ok" ]]; then
      mark="✓"; color="$GREEN"
    else
      mark="✗"; color="$RED"; all_ok=false
    fi

    printf "${CYAN}║${RESET} ${color}${mark}${RESET} %-44s ${DIM}%4ss${RESET} %*s${CYAN}║${RESET}\n" \
      "$(printf '%.44s' "$label")" "$dur" $(( width - 54 )) ""
  done

  echo -e "${CYAN}╠$(printf '═%.0s' $(seq 1 "$width"))╣${RESET}"

  if $all_ok; then
    printf "${CYAN}║${RESET}  ${BG_GREEN}${WHITE}${BOLD} ALL CHECKS PASSED ${RESET}%*s${CYAN}║${RESET}\n" $(( width - 21 )) ""
    if $RUN_INSTALL; then
      local devcli_bin pm_bin
      devcli_bin="$(command -v devcli 2>/dev/null || echo "${HOME}/.cargo/bin/devcli")"
      pm_bin="$(command -v pm-daemon 2>/dev/null || echo "${HOME}/.cargo/bin/pm-daemon")"
      printf "${CYAN}║${RESET}  ${DIM}devcli${RESET}     %-44s${CYAN}║${RESET}\n" "$devcli_bin"
      printf "${CYAN}║${RESET}  ${DIM}pm-daemon${RESET}  %-44s${CYAN}║${RESET}\n" "$pm_bin"
    fi
  else
    printf "${CYAN}║${RESET}  ${BG_RED}${WHITE}${BOLD} SOME STEPS FAILED ${RESET}%*s${CYAN}║${RESET}\n" $(( width - 20 )) ""
  fi

  echo -e "${CYAN}╚$(printf '═%.0s' $(seq 1 "$width"))╝${RESET}"
  echo

  $all_ok
}

main() {
  require_cmd rtk
  require_cmd cargo
  require_cmd rustc

  STEP_TOTAL="$(count_planned_steps)"
  if [[ "$STEP_TOTAL" -eq 0 ]]; then
    echo "Nothing to do (all steps skipped)." >&2
    exit 0
  fi

  print_banner
  print_context

  local idx=0
  local failed=false

  if $RUN_TESTS; then
    idx=$((idx + 1))
    if ! run_rtk_step "$idx" "Workspace tests" \
      rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo test --workspace --all-features; then
      failed=true
    fi
  fi

  if $RUN_CLIPPY; then
    idx=$((idx + 1))
    if ! run_rtk_step "$idx" "Clippy (deny warnings)" \
      rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo clippy --workspace --all-features --all-targets -- -D warnings; then
      failed=true
    fi
  fi

  if $RUN_INSTALL; then
    if ! $failed; then
      idx=$((idx + 1))
      if ! run_rtk_step "$idx" "Install devcli" \
        rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo install --path devcli --force; then
        failed=true
      fi

      idx=$((idx + 1))
      if ! run_rtk_step "$idx" "Install pm-daemon" \
        rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo install --path process-manager --force; then
        failed=true
      fi
    else
      echo -e "${YELLOW}⚠${RESET} Skipping install — fix failing checks first."
      echo
    fi
  fi

  if print_summary; then
    exit 0
  fi
  exit 1
}

main "$@"

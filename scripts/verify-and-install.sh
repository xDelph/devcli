#!/usr/bin/env bash
#
# verify-and-install.sh — Run workspace checks via RTK, then install devcli + pm-daemon.
#
# Usage:
#   ./scripts/verify-and-install.sh              # per-crate tests + clippy + install
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
declare -a WORKSPACE_CRATES=()

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
BOX_WIDTH=62
declare -a STEP_NAMES=()
declare -a STEP_STATUS=()
declare -a STEP_DETAIL=()
declare -a STEP_DURATION=()
declare -a TESTED_CRATES=()

strip_ansi() {
  printf '%s' "$1" | sed $'s/\x1B\[[0-9;]*[[:alpha:]]//g'
}

# Print one summary box row: visible content is padded or truncated to BOX_WIDTH.
box_row() {
  local content="$1"
  local visible len pad

  visible="$(strip_ansi "$content")"
  len=${#visible}
  if (( len > BOX_WIDTH )); then
    content="${visible:0:BOX_WIDTH}"
    len=$BOX_WIDTH
  fi
  pad=$(( BOX_WIDTH - len ))
  printf "${CYAN}║${RESET}%s%*s${CYAN}║${RESET}\n" "$content" "$pad" ""
}

box_rule() {
  local left="$1"
  local right="${2:-$1}"
  echo -e "${CYAN}${left}$(printf '═%.0s' $(seq 1 "$BOX_WIDTH"))${right}${RESET}"
}

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

discover_workspace_crates() {
  local members_line crate
  members_line="$(grep -E '^members\s*=' "$REPO_ROOT/Cargo.toml" | head -1)"
  if [[ -z "$members_line" ]]; then
    echo -e "${RED}✗${RESET} Could not find workspace members in Cargo.toml" >&2
    exit 1
  fi

  WORKSPACE_CRATES=()
  # shellcheck disable=SC2206
  local raw=($(echo "$members_line" | sed 's/members = \[//; s/\].*//; s/"//g; s/,/ /g'))
  for crate in "${raw[@]}"; do
    crate="${crate// /}"
    [[ -n "$crate" ]] && WORKSPACE_CRATES+=("$crate")
  done

  if [[ ${#WORKSPACE_CRATES[@]} -eq 0 ]]; then
    echo -e "${RED}✗${RESET} No workspace crates discovered" >&2
    exit 1
  fi
}

crate_label() {
  case "$1" in
    app-detector) echo "App detector" ;;
    config-manager) echo "Config manager" ;;
    devcli) echo "CLI (devcli)" ;;
    devcli-core) echo "Core (devcli-core)" ;;
    env-flow) echo "Env flow" ;;
    process-manager) echo "Process manager" ;;
    *) echo "$1" ;;
  esac
}

print_banner() {
  local title="devcli · verify & install"
  local subtitle="RTK-powered per-crate checks + local install"
  local pad_title=$(( (BOX_WIDTH - ${#title} - 2) / 2 ))

  echo
  box_rule "╔" "╗"
  printf "${CYAN}║${RESET}%*s${BOLD}${WHITE}%s${RESET}%*s${CYAN}║${RESET}\n" \
    "$pad_title" "" "$title" "$(( BOX_WIDTH - pad_title - ${#title} ))" ""
  local pad_sub=$(( (BOX_WIDTH - ${#subtitle} - 2) / 2 ))
  printf "${CYAN}║${RESET}%*s${DIM}%s${RESET}%*s${CYAN}║${RESET}\n" \
    "$pad_sub" "" "$subtitle" "$(( BOX_WIDTH - pad_sub - ${#subtitle} ))" ""
  box_rule "╚" "╝"
  echo
}

print_context() {
  local crate_list
  crate_list="$(printf '%s, ' "${WORKSPACE_CRATES[@]}")"
  crate_list="${crate_list%, }"

  echo -e "${DIM}repo${RESET}    ${BLUE}${REPO_ROOT}${RESET}"
  echo -e "${DIM}rtk${RESET}     $(rtk --version 2>/dev/null | head -1 || echo "rtk")"
  echo -e "${DIM}rust${RESET}    $(rustc --version 2>/dev/null || echo "unknown")"
  echo -e "${DIM}crates${RESET}  ${crate_list} ${DIM}(${#WORKSPACE_CRATES[@]})${RESET}"
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
  local icon color

  if [[ "$status" == "ok" ]]; then
    icon="✓"; color="$GREEN"
  else
    icon="✗"; color="$RED"
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
  if $RUN_TESTS; then
    n=$((n + ${#WORKSPACE_CRATES[@]}))
  fi
  $RUN_CLIPPY && n=$((n + 1))
  $RUN_INSTALL && n=$((n + 2))
  echo "$n"
}

verify_all_crates_tested() {
  local crate
  local missing=()

  for crate in "${WORKSPACE_CRATES[@]}"; do
    local found=false
    local tested
    for tested in "${TESTED_CRATES[@]}"; do
      if [[ "$tested" == "$crate" ]]; then
        found=true
        break
      fi
    done
    $found || missing+=("$crate")
  done

  if [[ ${#missing[@]} -gt 0 ]]; then
    echo -e "${RED}✗${RESET} Missing test runs for: ${missing[*]}" >&2
    return 1
  fi

  if [[ ${#TESTED_CRATES[@]} -ne ${#WORKSPACE_CRATES[@]} ]]; then
    echo -e "${RED}✗${RESET} Crate coverage mismatch: tested ${#TESTED_CRATES[@]} / ${#WORKSPACE_CRATES[@]}" >&2
    return 1
  fi

  return 0
}

print_summary() {
  local all_ok=true
  local i
  local max_path=$(( BOX_WIDTH - 13 ))

  box_rule "╔" "╗"
  box_row "  ${BOLD}${WHITE}Summary${RESET}"
  box_rule "╠" "╣"

  for i in "${!STEP_NAMES[@]}"; do
    local label="${STEP_DETAIL[$i]}"
    local dur="${STEP_DURATION[$i]}"
    local mark color

    if [[ "${STEP_STATUS[$i]}" == "ok" ]]; then
      mark="✓"; color="$GREEN"
    else
      mark="✗"; color="$RED"; all_ok=false
    fi

    box_row " ${color}${mark}${RESET} $(printf '%.44s' "$label") ${DIM}$(printf '%4s' "$dur")s${RESET}"
  done

  box_rule "╠" "╣"

  if $RUN_TESTS && [[ ${#TESTED_CRATES[@]} -gt 0 ]]; then
    box_row "  ${DIM}Test coverage:${RESET} ${#TESTED_CRATES[@]}/${#WORKSPACE_CRATES[@]} workspace crates"
  fi

  if $all_ok; then
    box_row "  ${BG_GREEN}${WHITE}${BOLD} ALL CHECKS PASSED ${RESET}"
    if $RUN_INSTALL; then
      local devcli_bin pm_bin
      devcli_bin="$(command -v devcli 2>/dev/null || echo "${HOME}/.cargo/bin/devcli")"
      pm_bin="$(command -v pm-daemon 2>/dev/null || echo "${HOME}/.cargo/bin/pm-daemon")"
      box_row "  ${DIM}devcli${RESET}     $(printf "%.${max_path}s" "$devcli_bin")"
      box_row "  ${DIM}pm-daemon${RESET}  $(printf "%.${max_path}s" "$pm_bin")"
    fi
  else
    box_row "  ${BG_RED}${WHITE}${BOLD} SOME STEPS FAILED ${RESET}"
  fi

  box_rule "╚" "╝"
  echo

  $all_ok
}

main() {
  require_cmd rtk
  require_cmd cargo
  require_cmd rustc

  discover_workspace_crates

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
    local crate
    for crate in "${WORKSPACE_CRATES[@]}"; do
      idx=$((idx + 1))
      if run_rtk_step "$idx" "Tests · $(crate_label "$crate")" \
        rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo test -p "$crate" --all-features; then
        TESTED_CRATES+=("$crate")
      else
        failed=true
      fi
    done

    if ! verify_all_crates_tested; then
      failed=true
    fi
  fi

  if $RUN_CLIPPY; then
    idx=$((idx + 1))
    if ! run_rtk_step "$idx" "Clippy · workspace" \
      rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo clippy --workspace --all-features --all-targets -- -D warnings; then
      failed=true
    fi
  fi

  if $RUN_INSTALL; then
    if ! $failed; then
      idx=$((idx + 1))
      if ! run_rtk_step "$idx" "Install · devcli" \
        rtk "${RTK_VERBOSE[@]+"${RTK_VERBOSE[@]}"}" cargo install --path devcli --force; then
        failed=true
      fi

      idx=$((idx + 1))
      if ! run_rtk_step "$idx" "Install · pm-daemon" \
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

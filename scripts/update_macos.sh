#!/bin/sh
set -u

pid=$1
dmg=$2
app=$3
log=$4
verify_mode=0
if [ "$#" -eq 5 ] && [ "$5" = "--verify-update" ]; then
    verify_mode=1
elif [ "$#" -ne 4 ]; then
    exit 2
fi
parent=${app%/*}
mount=
stage="$parent/.AlphaDocs.update-$pid"
backup="$parent/AlphaDocs.previous-$pid"
failed="$parent/AlphaDocs.failed-$pid"
profile=
backup_ready=0

say() { printf '%s\n' "$*" >> "$log"; }
fail() {
    say "Update failed: $*"
    if [ "$verify_mode" -eq 0 ] && ! kill -0 "$pid" 2>/dev/null; then
        resume="$app"
        if [ "$backup_ready" -eq 1 ] && [ -x "$backup/Contents/MacOS/alphadocs" ]; then resume="$backup"; fi
        if [ -x "$resume/Contents/MacOS/alphadocs" ]; then open -a "$resume" >> "$log" 2>&1 || true; fi
    fi
    if [ "$verify_mode" -eq 0 ]; then
        osascript -e 'display alert "AlphaDocs update failed" message "The existing application was preserved. See the update log in AlphaDocs application data." as critical' >/dev/null 2>&1 || true
    fi
    exit 1
}
check_app() {
    check_exe=$1
    ALPHADOCS_DATA_DIR="$profile" "$check_exe" --self-check >> "$log" 2>&1 &
    check_pid=$!
    waited=0
    while kill -0 "$check_pid" 2>/dev/null; do
        if [ "$waited" -ge 120 ]; then
            kill "$check_pid" 2>/dev/null || true
            wait "$check_pid" 2>/dev/null || true
            return 1
        fi
        sleep 1
        waited=$((waited + 1))
    done
    wait "$check_pid"
}

say "Waiting for AlphaDocs process $pid to exit."
waited=0
while kill -0 "$pid" 2>/dev/null; do
    [ "$waited" -lt 300 ] || fail "AlphaDocs did not close within five minutes. No update files were changed."
    sleep 1
    waited=$((waited + 1))
done

if [ "$verify_mode" -eq 1 ]; then
    [ -n "${RUNNER_TEMP:-}" ] || fail "Isolated update verification requires RUNNER_TEMP."
    temp_real=$(cd "$RUNNER_TEMP" 2>/dev/null && pwd -P) || fail "Could not resolve the isolated temporary folder."
    parent_real=$(cd "$parent" 2>/dev/null && pwd -P) || fail "Isolated update target parent must exist."
    [ "$parent_real" = "$temp_real" ] || fail "Isolated update verification app must be directly inside RUNNER_TEMP."
    [ -d "$app" ] || fail "Isolated update target bundle must already exist."
fi

if [ "$verify_mode" -eq 1 ]; then
    mount="${RUNNER_TEMP:?}/alphadocs-update-mount-$pid"
    mkdir -m 700 "$mount" || fail "could not create the isolated disk image mount folder"
else
    mount=$(mktemp -d "${TMPDIR:-/tmp}/alphadocs-update-mount.XXXXXX") || fail "could not create a temporary disk image mount folder"
fi
mounted=0
cleanup() {
    if [ "$mounted" -eq 1 ]; then hdiutil detach "$mount" >/dev/null 2>&1 || true; fi
    rmdir "$mount" >/dev/null 2>&1 || true
    if [ -n "$profile" ]; then rmdir "$profile" >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT
trap 'fail "update helper was interrupted"' HUP INT TERM

hdiutil attach -readonly -nobrowse -mountpoint "$mount" "$dmg" >> "$log" 2>&1 || fail "could not mount the verified disk image"
mounted=1
[ -d "$mount/AlphaDocs.app" ] || fail "disk image does not contain AlphaDocs.app"
[ ! -e "$stage" ] && [ ! -L "$stage" ] || fail "staging path already exists"
ditto "$mount/AlphaDocs.app" "$stage" >> "$log" 2>&1 || fail "could not stage the application bundle"
codesign --verify --deep --strict "$stage" >> "$log" 2>&1 || fail "the staged application signature is invalid"
profile=$(mktemp -d "${TMPDIR:-/tmp}/alphadocs-update-profile.XXXXXX") || fail "could not create a temporary validation profile"
check_app "$stage/Contents/MacOS/alphadocs" || fail "the staged application failed its native self-check"

[ ! -e "$failed" ] && [ ! -L "$failed" ] || fail "a retained failed-update path already exists; no installed bundle was changed"
[ ! -e "$backup" ] && [ ! -L "$backup" ] || fail "backup path already exists"
mv "$app" "$backup" || fail "could not retain the current application"
backup_ready=1
if ! mv "$stage" "$app"; then
    mv "$backup" "$app" || fail "replacement and rollback both failed; previous app remains at $backup"
    fail "could not install the staged application"
fi
if ! codesign --verify --deep --strict "$app" >> "$log" 2>&1 || ! check_app "$app/Contents/MacOS/alphadocs"; then
    mv "$app" "$failed" || fail "installed app validation failed; old app remains at $backup"
    mv "$backup" "$app" || fail "installed app validation and rollback failed; old app remains at $backup"
    fail "the installed application failed validation; the previous app was restored"
fi
if [ "$verify_mode" -eq 0 ] && ! open -a "$app"; then
    mv "$app" "$failed" || fail "the new app could not open; previous bundle remains at $backup"
    mv "$backup" "$app" || fail "the new app could not open and rollback failed; previous bundle remains at $backup"
    fail "the new app could not open; the previous version was restored"
fi
say "Update installed and validated. Previous bundle retained at $backup."

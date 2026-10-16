#!/bin/sh
set -u

pid=$1
deb=$2
install_dir=$3
exe=$4
log=$5
verify_mode=0
if [ "$#" -eq 6 ] && [ "$6" = "--verify-update" ]; then
    verify_mode=1
elif [ "$#" -ne 5 ]; then
    exit 2
fi
if [ "$verify_mode" -eq 1 ]; then
    work="${RUNNER_TEMP:?}/alphadocs-update-work-$pid"
else
    work="${deb%/*}/install-work-$pid"
fi
payload="$work/opt/alphadocs"
profile="$work/profile"
backup="$install_dir.previous-$pid"
failed="$install_dir.failed-$pid"
backup_ready=0

say() { printf '%s\n' "$*" >> "$log"; }
notify_error() {
    say "Update failed: $*"
    # The timeout path can happen while the original app is still running.
    # Avoid starting a second copy in that case; otherwise resume whichever
    # validated old binary is available after a failed handoff.
    native_relaunch=0
    if [ "$verify_mode" -eq 0 ] && ! kill -0 "$pid" 2>/dev/null; then
        resume="$exe"
        if [ "$backup_ready" -eq 1 ] && [ -x "$backup/alphadocs" ]; then resume="$backup/alphadocs"; fi
        if [ -x "$resume" ]; then
            "$resume" --update-error "$log" >> "$log" 2>&1 &
            native_relaunch=1
        fi
    fi
    if [ "$verify_mode" -eq 1 ] || [ "$native_relaunch" -eq 1 ]; then
        :
    elif command -v zenity >/dev/null 2>&1; then
        zenity --error --title="AlphaDocs update failed" --text="$*\nThe existing installation was preserved where possible. See the update log." >/dev/null 2>&1 || true
    elif command -v kdialog >/dev/null 2>&1; then
        kdialog --error "$*\nThe existing installation was preserved where possible. See the update log." --title "AlphaDocs update failed" >/dev/null 2>&1 || true
    elif command -v notify-send >/dev/null 2>&1; then
        notify-send "AlphaDocs update failed" "$*" >/dev/null 2>&1 || true
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
    [ "$waited" -lt 300 ] || notify_error "AlphaDocs did not close within five minutes. No update files were changed."
    sleep 1
    waited=$((waited + 1))
done

if [ "$verify_mode" -eq 1 ]; then
    [ -n "${RUNNER_TEMP:-}" ] || notify_error "Isolated update verification requires RUNNER_TEMP."
    temp_real=$(cd "$RUNNER_TEMP" 2>/dev/null && pwd -P) || notify_error "Could not resolve the isolated temporary folder."
    install_real=$(cd "$install_dir" 2>/dev/null && pwd -P) || notify_error "Isolated update target must already exist."
    case "$install_real" in
        "$temp_real"/*) ;;
        *) notify_error "Isolated update verification target must be inside RUNNER_TEMP." ;;
    esac
    [ "$exe" = "$install_dir/alphadocs" ] || notify_error "Isolated executable path does not match the target folder."
fi

[ -x "$exe" ] || notify_error "The installed AlphaDocs executable is missing."
command -v dpkg-deb >/dev/null 2>&1 || notify_error "dpkg-deb is required to validate this update."
[ ! -e "$work" ] && [ ! -L "$work" ] || notify_error "The update work path already exists."
mkdir -m 700 "$work" || notify_error "Could not create a private validation folder."
dpkg-deb --extract "$deb" "$work" >> "$log" 2>&1 || notify_error "The downloaded Debian package could not be extracted."
[ -x "$payload/alphadocs" ] || notify_error "The package does not contain the expected application."
mkdir -m 700 "$profile" || notify_error "Could not create a temporary validation profile."
check_app "$payload/alphadocs" || notify_error "The extracted application failed its native self-check."
[ ! -e "$failed" ] && [ ! -L "$failed" ] || notify_error "A retained failed-update path already exists; no installed files were changed."

if [ "$install_dir" = "/opt/alphadocs" ]; then
    [ ! -e "$backup" ] && [ ! -L "$backup" ] || notify_error "The previous-version backup path already exists."
    # Preserve the complete old directory before the package manager is asked
    # to authorize and apply the verified local package.
    if command -v pkcon >/dev/null 2>&1; then
        command -v pkexec >/dev/null 2>&1 || notify_error "PackageKit is present but pkexec is missing; no changes were made."
        pkexec cp -a "$install_dir" "$backup" >> "$log" 2>&1 || notify_error "Authorization for the safety backup was declined or failed."
        backup_ready=1
        if ! pkcon -y install-local "$deb" >> "$log" 2>&1; then
            if [ -e "$install_dir" ]; then pkexec mv "$install_dir" "$failed" >> "$log" 2>&1 || notify_error "Package installation failed; old files remain at $backup."; fi
            pkexec mv "$backup" "$install_dir" >> "$log" 2>&1 || notify_error "Package installation and rollback failed; old files remain at $backup."
            notify_error "The package manager did not install the update. The prior application files were restored."
        fi
    elif command -v pkexec >/dev/null 2>&1 && command -v dpkg >/dev/null 2>&1; then
        pkexec cp -a "$install_dir" "$backup" >> "$log" 2>&1 || notify_error "Authorization for the safety backup was declined or failed."
        backup_ready=1
        if ! pkexec dpkg --install "$deb" >> "$log" 2>&1; then
            if [ -e "$install_dir" ]; then pkexec mv "$install_dir" "$failed" >> "$log" 2>&1 || notify_error "Package installation failed; old files remain at $backup."; fi
            pkexec mv "$backup" "$install_dir" >> "$log" 2>&1 || notify_error "Package installation and rollback failed; old files remain at $backup."
            notify_error "The package manager did not install the update. The prior application files were restored."
        fi
    else
        notify_error "Install PackageKit with pkexec or use your distribution's package manager. AlphaDocs was not changed."
    fi
else
    [ ! -e "$backup" ] && [ ! -L "$backup" ] || notify_error "The previous-version backup path already exists."
    mv "$install_dir" "$backup" || notify_error "Could not retain the current installation."
    backup_ready=1
    if ! mv "$payload" "$install_dir"; then
        mv "$backup" "$install_dir" || notify_error "The replacement failed; old files remain at $backup."
        notify_error "Could not place the validated application in its installation folder."
    fi
fi

if ! check_app "$exe"; then
    if [ "$install_dir" = "/opt/alphadocs" ]; then
        if [ -e "$install_dir" ]; then pkexec mv "$install_dir" "$failed" >> "$log" 2>&1 || notify_error "Installed validation failed; old files remain at $backup."; fi
        if [ -e "$backup" ]; then pkexec mv "$backup" "$install_dir" >> "$log" 2>&1 || notify_error "Installed validation and rollback failed; old files remain at $backup."; fi
    else
        if [ -e "$install_dir" ]; then mv "$install_dir" "$failed" || notify_error "Installed validation failed; old files remain at $backup."; fi
        if [ -e "$backup" ]; then mv "$backup" "$install_dir" || notify_error "Installed validation and rollback failed; old files remain at $backup."; fi
    fi
    notify_error "The installed application failed validation. The previous version was restored."
fi
say "Update installed and validated. Previous package files retained at $backup."
if [ "$verify_mode" -eq 0 ]; then
    "$exe" >> "$log" 2>&1 &
fi

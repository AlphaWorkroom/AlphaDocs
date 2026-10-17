# Updates and recovery

AlphaDocs checks public GitHub release metadata at most once a day, or when Help → Check for updates is selected. Turn daily checks off in Preferences or Tools → Notification settings. Document contents are not uploaded by the updater.

From version 0.1.3, Download and install saves all open documents and shows a matching native download-progress dialog. Editing is temporarily blocked. Received/total bytes are shown; Cancel returns to the editor without running the installer. After download verification, the app closes while the platform installer runs. The full download must match the platform asset's size and SHA-256 supplied by GitHub before it can be launched. Only canonical AlphaWorkroom/AlphaDocs release assets and a bounded list of GitHub HTTPS asset hosts are accepted; credentials and cookies are not sent. Missing or invalid verification metadata disables installation.

The verified file is checked again before launch. The platform helper waits for the old app to close, validates/stages the new program, preserves the previous installation, applies the update and reopens AlphaDocs. Local documents/preferences/session data stay in the platform application-data directory.

## Installed layouts

- **Windows:** the per-user Setup installation under `%LOCALAPPDATA%/Programs/AlphaDocs`. The compact light/dark helper remains visible while installing and restarting. A failed validation keeps/restores the earlier installation where possible and reports the error. Portable or development executables use the public manual installer route.
- **macOS:** `AlphaDocs.app` in `/Applications` or `~/Applications`, with a user-writable parent folder. The app closes during installation; the background helper mounts the verified DMG read-only, stages the bundle, verifies its signature and native self-check, swaps it with a retained backup, then asks LaunchServices to reopen it. There is no visible installer progress window on macOS. Permission/signature/self-check failures preserve or restore the old bundle, reopen the old app where possible and show an error. No quarantine removal, Gatekeeper bypass or Apple-notarisation claim is made.
- **Linux:** the standard `/opt/alphadocs` DEB installation requires normal PackageKit/PolicyKit or pkexec/dpkg authorization. Supported user-writable installation folders use the validated package contents without elevation. The app closes while the background helper validates and installs; there is no installer progress window, though the operating system may show its authorization prompt. The distribution must provide the required package tools and an authorization agent. If these are unavailable or permission is declined, the helper reports failure and attempts to reopen the old app with a native AlphaDocs error dialog. Previous program files are retained; restoring program files is not a rollback of the distribution's package database.

No helper changes OS privacy or security settings. Installation permission dialogs remain under the user's control. A restart interrupts a hosted live-sharing session; peers retain their local copies and can be invited again after restart.

## Recovery and diagnostics

Staged downloads and `install.log` live under the AlphaDocs application-data `updates/stage-…` directory. Failed downloads are not installed. The helper retains the previous program directory/bundle beside the installed application as a recovery copy. When replacement or rollback fails, its log identifies the retained location; do not remove it before recovering the app. User documents are separate from these program folders.

The Windows/Linux/macOS download assets remain separate. GitHub native CI builds each platform, validates package content, and exercises isolated update replacement paths. Test-only verification modes require fresh temporary paths and suppress real relaunch, shortcuts/associations and authorization. These checks do not prove physical Mac UI/Gatekeeper approval or every Linux authorization configuration.

Older 0.1.2 builds omitted native browser-link support and cannot receive this binary fix retroactively. Download and run the 0.1.3 installer once; later releases can use its built-in progress updater.


Windows 0.1.6 additionally fixes update swaps started from an installed shortcut. The installer itself could inherit a working directory inside the installation and prevent `Directory.Move` even after AlphaDocs exited. New clients launch it from staging; the installer also resets its own working directory, covering updates started by older clients. Production `--verify-update` tests the actual wait/install path in a disposable directory, validates new/retained executables and skips user shortcuts, associations and GUI relaunch during verification.

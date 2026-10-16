// Offline per-user installer. No service, elevation, document migration or downloads.
using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Win32;

[assembly: AssemblyTitle("AlphaDocs Setup")]
[assembly: AssemblyCompany("Alpha Workroom")]
[assembly: AssemblyProduct("AlphaDocs")]
internal sealed class Setup : Form {
    readonly Button install = new Button();
    readonly CheckBox accept = new CheckBox();
    readonly CheckBox desktop = new CheckBox();
    readonly CheckBox associate = new CheckBox();
    readonly Label status = new Label();
    readonly string destination;
    readonly bool verificationMode;
    readonly string verificationDataDirectory;
    string previousInstall;
    bool destinationValidated;
    bool installing;
    bool completed;

    [STAThread] static void Main(string[] args) {
        // QA extracts to an explicit empty directory without touching shortcuts,
        // associations, installed files, user profiles or starting the app.
        if (args.Length == 2 && args[0] == "--verify-extract") {
            if (Directory.Exists(args[1])) throw new IOException("Verification directory must be new.");
            Extract(args[1]);
            return;
        }
        if (args.Length == 2 && args[0] == "--verify-update") {
            VerifyUpdate(args[1]);
            return;
        }
        if ((args.Length == 3 || (args.Length == 4 && args[3] == "--dark")) && args[0] == "--update") {
            int pid;
            if (!Int32.TryParse(args[1], out pid) || pid <= 0) throw new ArgumentException("Invalid AlphaDocs process identifier.");
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new UpdateWindow(pid, args[2], args.Length == 4));
            return;
        }
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new Setup());
    }
    internal Setup() : this(null, false) { }
    internal Setup(string verificationDestination, bool isVerification) {
        destination = verificationDestination ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "AlphaDocs");
        verificationMode = isVerification;
        verificationDataDirectory = isVerification ? destination + ".profile" : null;
        Text = "Install AlphaDocs by Alpha Workroom";
        ClientSize = new Size(680, 590);
        StartPosition = FormStartPosition.CenterScreen;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        Font = new Font("Segoe UI", 10);
        Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
        var title = new Label { Text = "AlphaDocs by Alpha Workroom", Font = new Font("Segoe UI", 19, FontStyle.Bold), AutoSize = true, Location = new Point(24, 20) };
        Controls.Add(title);
        Controls.Add(new Label { Text = "Free Use & Non-Monetisation Licence 1.1 — please read the complete terms.", UseMnemonic = false, AutoSize = true, Location = new Point(24, 64) });
        var terms = new TextBox { Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Vertical, Location = new Point(24, 96), Size = new Size(632, 260), BackColor = Color.White };
        using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("AlphaDocs.Package.zip"))
        using (var zip = new ZipArchive(stream, ZipArchiveMode.Read))
        using (var reader = new StreamReader(zip.GetEntry("LICENSE").Open())) terms.Text = reader.ReadToEnd().Replace("\r\n", "\n").Replace("\n", Environment.NewLine);
        Controls.Add(terms);
        accept.Text = "I accept the supplied licence terms";
        accept.SetBounds(24, 370, 620, 26);
        accept.CheckedChanged += delegate { install.Enabled = accept.Checked && !installing; };
        Controls.Add(accept);
        desktop.Text = "Create a desktop shortcut"; desktop.Checked = true; desktop.SetBounds(24, 402, 620, 26); Controls.Add(desktop);
        associate.Text = "Add AlphaDocs to Open with for .adocs files"; associate.Checked = true; associate.SetBounds(24, 434, 620, 26); Controls.Add(associate);
        Controls.Add(new Label { Text = "Install for this Windows user: " + destination, AutoSize = false, Location = new Point(24, 470), Size = new Size(632, 40) });
        status.SetBounds(24, 516, 480, 54); Controls.Add(status);
        install.Text = "Install"; install.Enabled = false; install.SetBounds(536, 528, 120, 38); Controls.Add(install);
        install.Click += async delegate {
            if (completed) { Close(); return; }
            installing = true; install.Enabled = false; accept.Enabled = desktop.Enabled = associate.Enabled = false;
            status.Text = "Installing locally…";
            try {
                var shortcut = desktop.Checked; var association = associate.Checked;
                await Task.Run(delegate { Install(shortcut, association); });
                status.Text = "Installed. Start AlphaDocs from the Start menu.";
                install.Text = "Done";
                completed = true;
                install.Enabled = true;
            } catch (Exception error) {
                status.Text = "Install failed. " + error.Message;
                accept.Enabled = desktop.Enabled = associate.Enabled = true;
                install.Enabled = true;
            } finally { installing = false; }
        };
        FormClosing += delegate(object sender, FormClosingEventArgs e) { if (installing) e.Cancel = true; };
        Shown += delegate { terms.Select(0, 0); accept.Focus(); };
    }
    static void Extract(string directory) {
        var root = Path.GetFullPath(directory) + Path.DirectorySeparatorChar;
        Directory.CreateDirectory(root);
        using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("AlphaDocs.Package.zip"))
        using (var zip = new ZipArchive(stream, ZipArchiveMode.Read)) {
            foreach (var entry in zip.Entries) {
                var file = Path.GetFullPath(Path.Combine(root, entry.FullName));
                if (!file.StartsWith(root, StringComparison.OrdinalIgnoreCase)) throw new IOException("Invalid package path.");
                Directory.CreateDirectory(Path.GetDirectoryName(file));
                using (var input = entry.Open())
                using (var output = new FileStream(file, FileMode.CreateNew, FileAccess.Write)) input.CopyTo(output);
            }
        }
    }
    string Install(bool desktopShortcut, bool association) {
        destinationValidated = false;
        var stage = destination + ".install-" + Guid.NewGuid().ToString("N");
        Extract(stage);
        ValidateForInstall(Path.Combine(stage, "AlphaDocs.exe"));
        // Retain a previous installation rather than recursively deleting it.
        string previous = null;
        if (Directory.Exists(destination)) {
            foreach (var process in Process.GetProcessesByName("AlphaDocs")) {
                try { if (string.Equals(process.MainModule.FileName, Path.Combine(destination, "AlphaDocs.exe"), StringComparison.OrdinalIgnoreCase)) throw new IOException("Close the installed AlphaDocs and try again."); }
                catch (System.ComponentModel.Win32Exception) { }
            }
            previous = destination + ".previous-" + Guid.NewGuid().ToString("N");
            previousInstall = previous;
            Directory.Move(destination, previous);
        }
        try { Directory.Move(stage, destination); }
        catch {
            if (previous != null && !Directory.Exists(destination)) Directory.Move(previous, destination);
            throw;
        }
        var exe = Path.Combine(destination, "AlphaDocs.exe");
        try { ValidateForInstall(exe); }
        catch {
            var failed = destination + ".failed-" + Guid.NewGuid().ToString("N");
            Directory.Move(destination, failed);
            if (previous != null) Directory.Move(previous, destination);
            throw;
        }
        destinationValidated = true;
        if (!verificationMode) {
            var start = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), "AlphaDocs.lnk");
            Shortcut(start, exe);
            if (desktopShortcut) Shortcut(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory), "AlphaDocs.lnk"), exe);
            if (association) {
                using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\AlphaDocs.Document")) key.SetValue("", "AlphaDocs document");
                using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\AlphaDocs.Document\DefaultIcon")) key.SetValue("", "\"" + exe + "\",0");
                using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\AlphaDocs.Document\shell\open\command")) key.SetValue("", "\"" + exe + "\" \"%1\"");
                using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\.adocs\OpenWithProgids")) key.SetValue("AlphaDocs.Document", "");
            }
        }
        return previous;
    }
    static void VerifyUpdate(string requestedDestination) {
        var destination = Path.GetFullPath(requestedDestination);
        if (Directory.Exists(destination) || File.Exists(destination))
            throw new IOException("Update verification target must be a new, empty path.");
        var parent = Path.GetDirectoryName(destination);
        var temp = Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (String.IsNullOrEmpty(parent) || !String.Equals(Path.GetFullPath(parent), temp, StringComparison.OrdinalIgnoreCase) || !Directory.Exists(parent))
            throw new IOException("Update verification target must be a new direct child of the current user's temporary folder.");
        Extract(destination); // seed the isolated destination with the packaged old payload
        var setup = new Setup(destination, true);
        Directory.CreateDirectory(setup.verificationDataDirectory);
        var backup = setup.Install(false, false);
        if (String.IsNullOrEmpty(backup) || !File.Exists(Path.Combine(backup, "AlphaDocs.exe")))
            throw new IOException("Update verification did not retain the previous payload.");
        ValidatePayload(Path.Combine(backup, "AlphaDocs.exe"), setup.verificationDataDirectory);
        ValidatePayload(Path.Combine(destination, "AlphaDocs.exe"), setup.verificationDataDirectory);
    }
    void ValidateForInstall(string exe) { ValidatePayload(exe, verificationDataDirectory); }
    static void ValidatePayload(string exe, string dataDirectory = null) {
        var start = new ProcessStartInfo(exe, "--self-check") { UseShellExecute = false, CreateNoWindow = true };
        if (dataDirectory != null) {
            Directory.CreateDirectory(dataDirectory);
            start.EnvironmentVariables["ALPHADOCS_DATA_DIR"] = dataDirectory;
        }
        using (var process = Process.Start(start)) {
            if (process == null) throw new IOException("Could not start the staged AlphaDocs self-check.");
            if (!process.WaitForExit(120000)) {
                try { process.Kill(); } catch { }
                try { process.WaitForExit(); } catch { }
                throw new IOException("The staged AlphaDocs executable timed out during its native self-check.");
            }
            if (process.ExitCode != 0) throw new IOException("The staged AlphaDocs executable failed its native self-check.");
        }
    }
    internal void Update(int oldPid, string log) {
        AppendLog(log, "Waiting for AlphaDocs process " + oldPid + " to close.");
        try {
            using (var process = Process.GetProcessById(oldPid))
                if (!process.WaitForExit(300000)) throw new IOException("AlphaDocs did not close in time.");
        } catch (ArgumentException) { /* The application exited before this helper started. */ }
        AppendLog(log, "Validating and installing the verified update.");
        Install(false, false);
        AppendLog(log, "Installed update passed its native self-check.");
        Process.Start(new ProcessStartInfo(Path.Combine(destination, "AlphaDocs.exe")) { UseShellExecute = true });
    }
    static void AppendLog(string path, string message) {
        File.AppendAllText(path, DateTime.UtcNow.ToString("o") + " " + message + Environment.NewLine);
    }
    internal void RelaunchExisting(int oldPid) {
        try { if (!Process.GetProcessById(oldPid).HasExited) return; }
        catch (ArgumentException) { }
        var exe = Path.Combine(destination, "AlphaDocs.exe");
        var retained = previousInstall == null ? null : Path.Combine(previousInstall, "AlphaDocs.exe");
        if ((!destinationValidated || !File.Exists(exe)) && retained != null && File.Exists(retained)) exe = retained;
        if (File.Exists(exe)) Process.Start(new ProcessStartInfo(exe) { UseShellExecute = true });
    }
    static void Shortcut(string path, string executable) {
        var shellType = Type.GetTypeFromProgID("WScript.Shell");
        var shell = Activator.CreateInstance(shellType);
        var link = shellType.InvokeMember("CreateShortcut", BindingFlags.InvokeMethod, null, shell, new object[] { path });
        var type = link.GetType();
        type.InvokeMember("TargetPath", BindingFlags.SetProperty, null, link, new object[] { executable });
        type.InvokeMember("WorkingDirectory", BindingFlags.SetProperty, null, link, new object[] { Path.GetDirectoryName(executable) });
        type.InvokeMember("Save", BindingFlags.InvokeMethod, null, link, null);
    }
}
internal sealed class UpdateWindow : Form {
    readonly Setup setup;
    readonly int oldPid;
    readonly string log;
    readonly Label status = new Label();
    bool running;
    public UpdateWindow(int pid, string logPath, bool dark) {
        setup = new Setup();
        oldPid = pid;
        log = logPath;
        var surface = dark ? Color.FromArgb(32,35,41) : Color.FromArgb(248,250,253);
        var ink = dark ? Color.FromArgb(223,227,235) : Color.FromArgb(31,31,31);
        var muted = dark ? Color.FromArgb(157,164,177) : Color.FromArgb(95,99,104);
        var blue = dark ? Color.FromArgb(168,199,250) : Color.FromArgb(11,87,208);
        Text = "AlphaDocs";
        ClientSize = new Size(456, 244);
        StartPosition = FormStartPosition.CenterScreen;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = MinimizeBox = false;
        ControlBox = false;
        BackColor = surface;
        ForeColor = ink;
        Font = new Font("Segoe UI", 10.5f);
        Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
        var mark = new PictureBox { Image = Icon.ToBitmap(), SizeMode = PictureBoxSizeMode.Zoom, BackColor = surface };
        mark.SetBounds(24,24,32,36); Controls.Add(mark);
        var title = new Label { Text = "Updating AlphaDocs", Font = new Font("Segoe UI",18f), AutoSize = false, BackColor = surface, ForeColor = ink };
        title.SetBounds(70,26,362,36); Controls.Add(title);
        var note = new Label { Text = "Your documents are saved and will reopen.", ForeColor = muted, BackColor = surface };
        note.SetBounds(24,80,408,34); Controls.Add(note);
        status.Text = "Waiting for AlphaDocs to close…";
        status.BackColor = surface;
        status.ForeColor = ink;
        status.SetBounds(24,124,408,46); Controls.Add(status);
        var rail = new UpdateRail(blue, dark ? Color.FromArgb(71,77,88) : Color.FromArgb(216,225,236));
        rail.BackColor = surface; rail.SetBounds(24,184,408,8); Controls.Add(rail);
        var footer = new Label { Text = "Installing locally. No documents are uploaded.", Font = new Font("Segoe UI",9f), ForeColor = muted, BackColor = surface };
        footer.SetBounds(24,208,408,20); Controls.Add(footer);
        FormClosing += delegate(object sender, FormClosingEventArgs e) { if (running) e.Cancel = true; };
        Shown += async delegate {
            if (running) return;
            running = true;
            try {
                status.Text = "Verifying and installing your update…";
                await Task.Run(delegate { setup.Update(oldPid, log); });
                rail.Complete = true;
                status.Text = "Update installed. AlphaDocs has restarted.";
                await Task.Delay(800);
                running = false;
                Close();
            } catch (Exception error) {
                rail.Failed = true;
                status.ForeColor = dark ? Color.FromArgb(255,180,171) : Color.FromArgb(179,38,30);
                status.Text = "The update could not be installed.";
                try { File.AppendAllText(log, DateTime.UtcNow.ToString("o") + " Update failed: " + error + Environment.NewLine); } catch { }
                try { setup.RelaunchExisting(oldPid); } catch { }
                MessageBox.Show(this, "The previous installation was retained where possible.\r\n\r\n" + error.Message, "AlphaDocs update", MessageBoxButtons.OK, MessageBoxIcon.Error);
                running = false;
                Close();
            }
        };
    }
}
internal sealed class UpdateRail : Control {
    readonly Color accent;
    readonly Color track;
    readonly Timer clock = new Timer();
    public bool Complete;
    public bool Failed;
    public UpdateRail(Color blue, Color background) {
        accent = blue; track = background;
        SetStyle(ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint, true);
        clock.Interval = 30; clock.Tick += delegate { Invalidate(); }; clock.Start();
    }
    protected override void OnPaint(PaintEventArgs e) {
        base.OnPaint(e);
        e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        using (var pen = new Pen(track,6f)) { pen.StartCap = pen.EndCap = System.Drawing.Drawing2D.LineCap.Round; e.Graphics.DrawLine(pen,3,Height/2,Width-3,Height/2); }
        if (Failed) return;
        var phase = (Environment.TickCount & Int32.MaxValue) % 1800 / 1800f;
        var start = Complete ? 3f : phase * (Width+96) - 96;
        var end = Complete ? Width-3f : start+96;
        using (var pen = new Pen(accent,6f)) { pen.StartCap = pen.EndCap = System.Drawing.Drawing2D.LineCap.Round; e.Graphics.DrawLine(pen,start,Height/2,end,Height/2); }
    }
    protected override void Dispose(bool disposing) { if (disposing) clock.Dispose(); base.Dispose(disposing); }
}

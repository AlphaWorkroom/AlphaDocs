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
    readonly string destination = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "AlphaDocs");
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
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new Setup());
    }
    Setup() {
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
    void Install(bool desktopShortcut, bool association) {
        var stage = destination + ".install-" + Guid.NewGuid().ToString("N");
        Extract(stage);
        // Retain a previous installation rather than recursively deleting it.
        string previous = null;
        if (Directory.Exists(destination)) {
            foreach (var process in Process.GetProcessesByName("AlphaDocs")) {
                try { if (string.Equals(process.MainModule.FileName, Path.Combine(destination, "AlphaDocs.exe"), StringComparison.OrdinalIgnoreCase)) throw new IOException("Close the installed AlphaDocs and try again."); }
                catch (System.ComponentModel.Win32Exception) { }
            }
            previous = destination + ".previous-" + Guid.NewGuid().ToString("N");
            Directory.Move(destination, previous);
        }
        try { Directory.Move(stage, destination); }
        catch {
            if (previous != null && !Directory.Exists(destination)) Directory.Move(previous, destination);
            throw;
        }
        var exe = Path.Combine(destination, "AlphaDocs.exe");
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

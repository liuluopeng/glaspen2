// glaspen2 — Minimal Windows Installer
// Compiles with: csc /target:winexe /out:installer.exe Installer.cs
// Run: installer.exe (drag glaspen2 folder onto it, or put files next to it)

using System;
using System.IO;
using System.IO.Compression;
using System.Diagnostics;
using System.Windows.Forms;

class Glaspen2Installer
{
    [STAThread]
    static void Main()
    {
        string installDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "glaspen2");

        // Find the payload — either packed inside this exe, or next to it
        string exeDir = AppDomain.CurrentDomain.BaseDirectory;

        // Check for embedded payload (ZIP appended to this exe)
        string selfExe = Application.ExecutablePath;
        string payloadZip = Path.Combine(Path.GetTempPath(), "glaspen2_payload.zip");

        bool extracted = false;
        try
        {
            // Try to extract payload appended to this exe (self-extracting mode)
            ExtractEmbeddedPayload(selfExe, payloadZip);
            extracted = true;
        }
        catch
        {
            // Fallback: look for glaspen2 files next to this exe (portable mode)
            if (File.Exists(Path.Combine(exeDir, "glaspen2.exe")))
            {
                // Files already extracted next to the installer — just create shortcut
                CreateShortcut(exeDir);
                MessageBox.Show("glaspen2 shortcut created in Start Menu.\nRun glaspen2 from the Start Menu, or run glaspen2.exe directly.",
                    "glaspen2", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            // Look for glaspen2/ subdirectory
            string subDir = Path.Combine(exeDir, "glaspen2");
            if (Directory.Exists(subDir) && File.Exists(Path.Combine(subDir, "glaspen2.exe")))
            {
                CreateShortcut(subDir);
                MessageBox.Show("glaspen2 installed! Launch from Start Menu.",
                    "glaspen2", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            MessageBox.Show("glaspen2 files not found.\nPlace this installer next to the glaspen2 folder, or use the self-extracting package.",
                "glaspen2 Installer", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return;
        }

        if (extracted)
        {
            try
            {
                // Remove existing installation
                if (Directory.Exists(installDir))
                    Directory.Delete(installDir, true);

                Directory.CreateDirectory(installDir);

                // Unzip
                System.IO.Compression.ZipFile.ExtractToDirectory(payloadZip, installDir);

                // Create shortcut
                CreateShortcut(installDir);

                File.Delete(payloadZip);

                MessageBox.Show("glaspen2 installed successfully!\nLaunch from Start Menu → glaspen2",
                    "glaspen2 Installer", MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            catch (Exception ex)
            {
                MessageBox.Show("Installation failed: " + ex.Message,
                    "glaspen2 Installer", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }
    }

    static void ExtractEmbeddedPayload(string exePath, string outputZip)
    {
        // Read this exe, find ZIP magic bytes (PK\x03\x04) from the end
        byte[] exeBytes = File.ReadAllBytes(exePath);
        int zipStart = -1;
        for (int i = exeBytes.Length - 4; i >= 0; i--)
        {
            if (exeBytes[i] == 'P' && exeBytes[i+1] == 'K' &&
                exeBytes[i+2] == 0x03 && exeBytes[i+3] == 0x04)
            {
                zipStart = i;
                // Continue scanning backward for the FIRST PK header
            }
        }
        // Find the first PK header from the end
        for (int i = exeBytes.Length - 4; i >= 0; i--)
        {
            if (exeBytes[i] == 'P' && exeBytes[i+1] == 'K' &&
                exeBytes[i+2] == 0x03 && exeBytes[i+3] == 0x04)
            {
                zipStart = i;
            }
        }

        if (zipStart < 0)
            throw new Exception("No payload found in installer");

        int zipLen = exeBytes.Length - zipStart;
        using (var fs = new FileStream(outputZip, FileMode.Create))
        {
            fs.Write(exeBytes, zipStart, zipLen);
        }
    }

    static void CreateShortcut(string targetDir)
    {
        string targetExe = Path.Combine(targetDir, "glaspen2.exe");
        if (!File.Exists(targetExe)) return;

        Type t = Type.GetTypeFromProgID("WScript.Shell");
        dynamic shell = Activator.CreateInstance(t);
        string startMenu = Environment.GetFolderPath(Environment.SpecialFolder.Programs);
        string linkPath = Path.Combine(startMenu, "glaspen2.lnk");

        // Delete existing shortcut
        if (File.Exists(linkPath)) File.Delete(linkPath);

        dynamic shortcut = shell.CreateShortcut(linkPath);
        shortcut.TargetPath = targetExe;
        shortcut.WorkingDirectory = targetDir;
        shortcut.Description = "glaspen2 — screen pen overlay";
        shortcut.IconLocation = targetExe + ",0";
        shortcut.Save();
    }
}

# Window helpers for Voxa's own window (titled "Voxa"), dot-sourced by the measuring scripts.
if (-not ('OpenMeasure5.Win' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace OpenMeasure5 {
public static class Win {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid) {
        var t = new System.Text.StringBuilder(64); GetWindowText(h, t, 64);
        if (t.ToString() == "Voxa") { found = h; return false; }
      }
      return true; }, IntPtr.Zero);
    return found;
  }
  public static bool Shown(uint pid) { var h = Find(pid); return h != IntPtr.Zero && IsWindowVisible(h); }
  public static bool Minimised(uint pid) { var h = Find(pid); return h != IntPtr.Zero && IsIconic(h); }
  public static bool Close(uint pid) { var h = Find(pid); return h != IntPtr.Zero && PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }
  // Put Voxa's window behind the others without activating it.
  public static bool ToBottom(uint pid) { var h = Find(pid); return h != IntPtr.Zero && SetWindowPos(h, new IntPtr(1), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010); }

  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct STARTUPINFO {
    public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
    public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
    public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
  }
  [StructLayout(LayoutKind.Sequential)]
  struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
  [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags,
    IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  // Start a program whose first window is shown without being activated (SW_SHOWNOACTIVATE),
  // with this process's environment. Gives its process id.
  public static int StartQuietly(string exe, string args, string cwd) {
    var si = new STARTUPINFO(); si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
    si.dwFlags = 1; si.wShowWindow = 4;
    PROCESS_INFORMATION pi;
    string cmd = "\"" + exe + "\"" + (string.IsNullOrEmpty(args) ? "" : " \"" + args + "\"");
    if (!CreateProcess(exe, cmd, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, cwd, ref si, out pi))
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    CloseHandle(pi.hThread); CloseHandle(pi.hProcess);
    return pi.dwProcessId;
  }
}
}
'@
}

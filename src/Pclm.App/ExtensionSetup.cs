using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Automation;
using Microsoft.Win32;
using Pclm.Core.Erp;
using Pclm.Core.Storage;

namespace Pclm.App;

/// <summary>
/// 내장 확장과 브라우저 연결. <b>언제나 업무 홈에 둔다</b> — 레지스트리의 호스트 연결은 컴퓨터에 하나뿐이라,
/// <c>--home</c> 으로 띄운 시험 창이 그것을 고쳐 적으면 사람의 확장이 시험 홈에 묶인다. 그래서 부르는 쪽(창·다리)이
/// 업무 홈일 때만 부른다.
/// </summary>
internal static class ExtensionSetup
{
    public static BundledExtension.Prepared Prepare()
    {
        var result = BundledExtension.Prepare(Home.Default.Directory, Environment.ProcessPath!);
        foreach (var browser in new[] { @"Google\Chrome", @"Microsoft\Edge" })
        {
            using var key = Registry.CurrentUser.CreateSubKey($@"Software\{browser}\NativeMessagingHosts\{ErpConnection.HostName}");
            key.SetValue("", result.ManifestPath, RegistryValueKind.String);
        }
        return result;
    }

    /// <summary>
    /// 앱을 켤 때마다 이미 준비한 연결을 고친다. exe 를 새 판으로 바꾸거나 옮기면 풀어 둔 확장과
    /// 호스트 경로가 옛것을 가리키는데, 사람이 다시 준비하기 전까지는 아무도 알려 주지 않았다.
    /// 한 번도 준비하지 않은 사람의 레지스트리는 건드리지 않는다.
    /// </summary>
    public static void Refresh()
    {
        if (!Directory.Exists(Path.Combine(Home.Default.Directory, "extension"))) return;
        // 창을 막을 일이 아니다. 고치지 못했으면 설정 패널의 확장 상태가 옛 판을 드러낸다.
        try { Prepare(); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or InvalidOperationException) { }
    }

    /// <summary>
    /// 확장 폴더나 브라우저 안의 확장 화면을 연다. <paramref name="page"/> 는 <c>extensions/</c>(관리)·<c>extensions/shortcuts</c>(단축키).
    /// </summary>
    public static object? Open(string target, string page = "extensions/")
    {
        if (target == "folder")
        {
            var folder = Path.Combine(Home.Default.Directory, "extension");
            if (!Directory.Exists(folder)) throw new InvalidOperationException("먼저 내장 확장 준비를 누르세요.");
            Process.Start(new ProcessStartInfo(folder) { UseShellExecute = true });
            return null;
        }
        var (exe, scheme, relative) = target switch
        {
            "edge" => ("msedge.exe", "edge", @"Microsoft\Edge\Application\msedge.exe"),
            "chrome" => ("chrome.exe", "chrome", @"Google\Chrome\Application\chrome.exe"),
            _ => throw new ArgumentException("Edge 또는 Chrome을 선택하세요."),
        };
        var url = $"{scheme}://{page}";
        var candidates = new List<string?>();
        foreach (var root in new[] { Registry.CurrentUser, Registry.LocalMachine })
        {
            using var key = root.OpenSubKey($@"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
            candidates.Add((key?.GetValue("") as string)?.Trim('"'));
        }
        foreach (var root in new[] { Environment.SpecialFolder.LocalApplicationData, Environment.SpecialFolder.ProgramFiles, Environment.SpecialFolder.ProgramFilesX86 })
            candidates.Add(Path.Combine(Environment.GetFolderPath(root), relative));
        var path = candidates.FirstOrDefault(File.Exists)
            ?? throw new InvalidOperationException($"{(target == "edge" ? "Edge" : "Chrome")}를 찾지 못했습니다. 브라우저를 직접 열고 주소창에 {url}를 입력하세요.");
        // 브라우저는 명령줄로 받은 chrome://·edge:// 주소를 막고 새 탭만 연다(실측: Chrome·Edge 모두 「새 탭」). 새 창을 띄우고
        // 그 창의 주소창에 적어 넣는다. 확장이 붙어 있으면 이리 오지 않는다 — 다리가 확장에 맡긴다(openExtensions).
        var process = Path.GetFileNameWithoutExtension(exe);
        var before = Windows(target);
        var start = new ProcessStartInfo(path) { UseShellExecute = true };
        start.ArgumentList.Add("--new-window");
        Process.Start(start);
        if (!Navigate(process, before, url))
        {
            System.Windows.Application.Current.Dispatcher.Invoke(() => System.Windows.Clipboard.SetText(url));
            throw new InvalidOperationException($"브라우저 창에 주소를 넣지 못했습니다. {url} 을 복사해 두었으니 주소창에 붙여넣고 Enter 를 누르세요.");
        }
        return null;
    }

    /// <summary>그 브라우저(<c>chrome</c>·<c>edge</c>)의 지금 최상위 창. 뒤에 새로 선 창을 가리려고 미리 떠 둔다.</summary>
    public static HashSet<int> Windows(string target) => BrowserWindows(ProcessName(target)).Select(w => w.Current.NativeWindowHandle).ToHashSet();

    /// <summary>
    /// <paramref name="before"/> 뒤에 새로 선 그 브라우저의 창을 기다려 앞으로 가져온다. 서지 않았거나 앞으로 오지 않았으면 false.
    ///
    /// <para><b>확장이 연 창은 스스로 앞으로 오지 못한다.</b> 앞에 있는 것은 사람이 방금 누른 이 앱이고, 브라우저 프로세스는
    /// 입력을 받지 않아 Windows 가 포커스를 내주지 않는다 — 실측: 확장이 <c>windows.update({focused})</c> 를 불러도, 이 앱이
    /// <c>AllowSetForegroundWindow</c> 로 허락해도 창은 뒤에 남았다. 앞에 있는 이 앱이 올리면 올라온다.</para>
    /// </summary>
    public static bool Raise(string target, HashSet<int> before, TimeSpan wait)
    {
        var process = ProcessName(target);
        try
        {
            for (var waited = TimeSpan.Zero; waited < wait; waited += TimeSpan.FromMilliseconds(100))
            {
                Thread.Sleep(100);
                var window = BrowserWindows(process).FirstOrDefault(w => !before.Contains(w.Current.NativeWindowHandle));
                if (window is null) continue;
                var handle = new IntPtr(window.Current.NativeWindowHandle);
                SetForegroundWindow(handle);
                return GetForegroundWindow() == handle;
            }
        }
        catch (Exception e) when (e is ElementNotAvailableException or InvalidOperationException or COMException) { }
        return false;
    }

    private static string ProcessName(string target) => target switch
    {
        "edge" => "msedge",
        "chrome" => "chrome",
        _ => throw new ArgumentException("Edge 또는 Chrome을 선택하세요."),
    };

    /// <summary>그 브라우저의 최상위 창들.</summary>
    private static List<AutomationElement> BrowserWindows(string process)
    {
        var pids = Process.GetProcessesByName(process).Select(p => p.Id).ToHashSet();
        return AutomationElement.RootElement.FindAll(TreeScope.Children, Condition.TrueCondition).Cast<AutomationElement>()
            .Where(w => { try { return pids.Contains(w.Current.ProcessId); } catch (ElementNotAvailableException) { return false; } })
            .ToList();
    }

    /// <summary>
    /// 새로 뜬 창의 주소창에 주소를 적고 Enter. 그 창이 정말 앞에 왔을 때만 누른다 — 다른 창에 Enter 가 가면 안 된다.
    /// 앱이 앞에 있는 프로세스라 창을 앞으로 가져올 수 있다. 하나라도 어긋나면 false.
    /// </summary>
    private static bool Navigate(string process, HashSet<int> before, string url)
    {
        try
        {
            for (var tries = 0; tries < 50; tries++)
            {
                Thread.Sleep(100);
                var window = BrowserWindows(process).FirstOrDefault(w => !before.Contains(w.Current.NativeWindowHandle));
                if (window is null) continue;
                var omnibox = window.FindFirst(TreeScope.Descendants, new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Edit));
                if (omnibox is null || !omnibox.TryGetCurrentPattern(ValuePattern.Pattern, out var pattern)) continue;
                var handle = new IntPtr(window.Current.NativeWindowHandle);
                SetForegroundWindow(handle);
                omnibox.SetFocus();
                ((ValuePattern)pattern).SetValue(url);
                if (GetForegroundWindow() != handle) return false;
                keybd_event(0x0D, 0, 0, UIntPtr.Zero);         // VK_RETURN
                keybd_event(0x0D, 0, 0x0002, UIntPtr.Zero);    // KEYEVENTF_KEYUP
                return true;
            }
        }
        catch (Exception e) when (e is ElementNotAvailableException or InvalidOperationException or COMException) { }
        return false;
    }

    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] private static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
}

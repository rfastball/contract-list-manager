using System.Diagnostics;
using System.IO;
using Microsoft.Win32;
using Pclm.Core.Erp;

namespace Pclm.App;

internal static class ExtensionSetup
{
    public static BundledExtension.Prepared Prepare()
    {
        var result = BundledExtension.Prepare(ErpConnection.DirectoryPath, Environment.ProcessPath!);
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
        if (!Directory.Exists(Path.Combine(ErpConnection.DirectoryPath, "extension"))) return;
        // 창을 막을 일이 아니다. 고치지 못했으면 설정 패널의 확장 상태가 옛 판을 드러낸다.
        try { Prepare(); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or InvalidOperationException) { }
    }

    public static object? Open(string target)
    {
        if (target == "folder")
        {
            var folder = Path.Combine(ErpConnection.DirectoryPath, "extension");
            if (!Directory.Exists(folder)) throw new InvalidOperationException("먼저 내장 확장 준비를 누르세요.");
            Process.Start(new ProcessStartInfo(folder) { UseShellExecute = true });
            return null;
        }
        var (exe, url, relative) = target switch
        {
            "edge" => ("msedge.exe", "edge://extensions/", @"Microsoft\Edge\Application\msedge.exe"),
            "chrome" => ("chrome.exe", "chrome://extensions/", @"Google\Chrome\Application\chrome.exe"),
            _ => throw new ArgumentException("Edge 또는 Chrome을 선택하세요."),
        };
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
        var start = new ProcessStartInfo(path) { UseShellExecute = true };
        start.ArgumentList.Add(url);
        Process.Start(start);
        return null;
    }
}

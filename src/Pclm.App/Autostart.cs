using System.IO;
using Microsoft.Win32;

namespace Pclm.App;

/// <summary>
/// Windows 에 로그인하면 창을 켠다 — <c>HKCU\...\Run</c> 의 값 하나(<c>"exe" --background</c>).
///
/// <para><b>참은 레지스트리 하나다.</b> 켰는지를 따로 적어 두지 않는다 — 사람이 작업 관리자의 「시작 프로그램」 에서
/// 끄거나 값을 지우면 두 벌이 어긋나 화면은 켜졌다고 말하는데 켜지지 않는다. 작업 관리자가 끈 것은 값을 두고
/// <c>StartupApproved</c> 에 표시만 하므로 그것도 꺼짐으로 읽는다.</para>
///
/// <para><b>업무 홈에서만 다룬다</b>(<see cref="FileAssociation"/> 과 같은 까닭). Run 값은 컴퓨터에 하나라, <c>--home</c> 으로
/// 띄운 시험 창이 적으면 로그인할 때 시험 홈이 뜬다 — 적힌 명령에는 홈이 없으니 실제로는 업무 홈이 뜨는데 시험 창은
/// 그것을 켠 줄 안다.</para>
///
/// <para>배포물은 exe 하나라 설치기가 없다. exe 를 새 판으로 바꾸거나 옮기면 값이 옛 자리를 가리키므로, 업무 홈의 창이
/// 설 때마다 켜져 있으면 지금 자리로 고쳐 적는다(<see cref="Refresh"/>).</para>
/// </summary>
internal static class Autostart
{
    /// <summary>로그인할 때 켜졌다는 표시. 창을 띄우지 않거나 작업 표시줄에 내려 둔 채 선다.</summary>
    public const string BackgroundFlag = "--background";

    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ApprovedKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    private const string ValueName = "계약목록";

    /// <summary>자동 실행을 걸 수 없는 까닭. 걸 수 있으면 <c>null</c>.</summary>
    public static string? Unavailable(bool isDefaultHome)
    {
        if (!isDefaultHome)
            return "시험 홈(--home)으로 띄운 창이라 자동 실행을 바꾸지 않습니다. 업무 홈의 창에서 바꾸세요.";
        if (Environment.ProcessPath is not { } exe)
            return "이 프로그램의 자리를 알 수 없어 자동 실행을 걸 수 없습니다.";
        if (InTemp(exe))
            return "압축 파일 안에서 곧바로 켠 프로그램이라 자동 실행을 걸 수 없습니다. 압축을 푼 자리에서 켜세요.";
        return null;
    }

    /// <summary>로그인할 때 켜지는가. 값이 있고 작업 관리자가 끄지 않았어야 한다.</summary>
    public static bool IsEnabled()
    {
        try
        {
            using var run = Registry.CurrentUser.OpenSubKey(RunKey);
            return run?.GetValue(ValueName) is string && !TurnedOffByUser();
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            return false;
        }
    }

    /// <summary>
    /// 켜거나 끈다. 작업 관리자가 꺼 둔 표시도 함께 걷는다 — 남겨 두면 여기서 켠 것이 다시 꺼진 채로 선다.
    /// 부르기 전에 <see cref="Unavailable"/> 을 본다.
    /// </summary>
    public static void Set(bool on)
    {
        using (var run = Registry.CurrentUser.CreateSubKey(RunKey))
        {
            if (on) run.SetValue(ValueName, Command(Environment.ProcessPath!));
            else run.DeleteValue(ValueName, throwOnMissingValue: false);
        }

        using var approved = Registry.CurrentUser.OpenSubKey(ApprovedKey, writable: true);
        approved?.DeleteValue(ValueName, throwOnMissingValue: false);
    }

    /// <summary>
    /// 켜져 있으면 지금 exe 자리로 고쳐 적는다. 꺼져 있으면 아무것도 적지 않는다 — 켜는 것은 사람만 한다.
    /// 작업 관리자가 꺼 둔 표시는 건드리지 않는다. 업무 홈에서만 부른다.
    /// </summary>
    public static void Refresh()
    {
        if (Environment.ProcessPath is not { } exe || InTemp(exe)) return;

        try
        {
            using var run = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (run?.GetValue(ValueName) is not string current) return;

            var command = Command(exe);
            if (current != command) run.SetValue(ValueName, command);
        }
        // 창을 막을 일이 아니다. 고쳐 적지 못했으면 다음 로그인에 옛 자리를 찾다 말 뿐이다.
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException) { }
    }

    private static string Command(string exe) => $"\"{exe}\" {BackgroundFlag}";

    /// <summary>
    /// 작업 관리자의 「사용 안 함」. 값을 지우지 않고 <c>StartupApproved</c> 에 첫 바이트가 홀수인 표시를 남긴다
    /// (짝수면 사용).
    /// </summary>
    private static bool TurnedOffByUser()
    {
        using var approved = Registry.CurrentUser.OpenSubKey(ApprovedKey);
        return approved?.GetValue(ValueName) is byte[] { Length: > 0 } mark && (mark[0] & 1) == 1;
    }

    /// <summary>
    /// 압축 파일 안에서 곧바로 켜면 탐색기가 임시 폴더에 풀어 띄운다. 그 자리는 곧 지워지니 적지 않는다
    /// (<see cref="FileAssociation.Refresh"/> 와 같은 판단).
    /// </summary>
    private static bool InTemp(string exe) =>
        Path.GetFullPath(exe).StartsWith(Path.GetFullPath(Path.GetTempPath()), StringComparison.OrdinalIgnoreCase);
}

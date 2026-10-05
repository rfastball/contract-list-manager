using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace Pclm.App;

/// <summary>
/// <c>.pclm</c> 을 이 exe 에 잇는다 — 받은 제출본을 두 번 누르면 이 프로그램을 몰라도 열람 창이 뜬다.
///
/// <para>배포물은 exe 하나라 설치기가 없다. 그래서 창을 켤 때마다 적는다: exe 를 새 판으로 바꾸거나 옮기면 옛 자리를
/// 가리키는 연결이 남는데, 다시 켜기 전까지는 아무도 알려 주지 않는다(<see cref="ExtensionSetup.Refresh"/> 와 같은 까닭).
/// 관리자 권한이 들지 않게 <c>HKCU\Software\Classes</c> 에만 적는다.</para>
///
/// <para><b>업무 홈에서만 부른다.</b> 연결은 컴퓨터에 하나뿐이라, <c>--home</c> 으로 띄운 시험 창이 고쳐 적으면
/// 사람이 두 번 누른 파일이 시험 홈의 창으로 간다.</para>
/// </summary>
internal static class FileAssociation
{
    private const string Extension = ".pclm";
    private const string ProgId = "Pclm.Workfile";

    public static void Refresh()
    {
        var exe = Environment.ProcessPath;
        if (exe is null) return;

        // 압축 파일 안에서 곧바로 켜면 탐색기가 임시 폴더에 풀어 띄운다. 그 자리는 곧 지워지니 적지 않는다 —
        // 적으면 두 번 누른 파일이 「찾을 수 없음」 으로 끝나고, 다음에 제자리에서 켤 때까지 그대로다.
        if (Path.GetFullPath(exe).StartsWith(Path.GetFullPath(Path.GetTempPath()), StringComparison.OrdinalIgnoreCase)) return;

        var command = $"\"{exe}\" \"%1\"";
        try
        {
            using (var current = Registry.CurrentUser.OpenSubKey($@"Software\Classes\{ProgId}\shell\open\command"))
            {
                if (current?.GetValue("") as string == command && HasDefault()) return;
            }

            using (var key = Registry.CurrentUser.CreateSubKey($@"Software\Classes\{Extension}"))
            {
                key.SetValue("", ProgId);
                using var openWith = key.CreateSubKey("OpenWithProgids");
                openWith.SetValue(ProgId, Array.Empty<byte>(), RegistryValueKind.None);
            }

            using (var key = Registry.CurrentUser.CreateSubKey($@"Software\Classes\{ProgId}"))
            {
                key.SetValue("", "계약 목록 자료");
                using (var icon = key.CreateSubKey("DefaultIcon")) icon.SetValue("", $"\"{exe}\",0");
                using (var open = key.CreateSubKey(@"shell\open\command")) open.SetValue("", command);
            }

            // 「연결 프로그램」 목록에 exe 파일 이름 대신 제 이름으로 서게 한다.
            using (var key = Registry.CurrentUser.CreateSubKey($@"Software\Classes\Applications\{Path.GetFileName(exe)}"))
            {
                key.SetValue("FriendlyAppName", "계약 목록");
                using (var types = key.CreateSubKey("SupportedTypes")) types.SetValue(Extension, "");
                using (var open = key.CreateSubKey(@"shell\open\command")) open.SetValue("", command);
            }

            // 알리지 않으면 탐색기가 다시 켜질 때까지 옛 연결과 아이콘을 쥐고 있다.
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, IntPtr.Zero, IntPtr.Zero);
        }
        // 창을 막을 일이 아니다. 적지 못했으면 두 번 누른 파일이 연결 프로그램을 물을 뿐이다.
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException) { }
    }

    /// <summary>확장자가 우리 ProgId 를 기본으로 가리키는가. 다른 프로그램이 그 자리를 가져갔으면 되찾는다.</summary>
    private static bool HasDefault()
    {
        using var key = Registry.CurrentUser.OpenSubKey($@"Software\Classes\{Extension}");
        return key?.GetValue("") as string == ProgId;
    }

    private const int SHCNE_ASSOCCHANGED = 0x08000000;
    private const uint SHCNF_IDLIST = 0x0000;

    [DllImport("shell32.dll")]
    private static extern void SHChangeNotify(int eventId, uint flags, IntPtr item1, IntPtr item2);
}

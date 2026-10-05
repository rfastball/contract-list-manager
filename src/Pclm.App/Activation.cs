using System.Runtime.InteropServices;
using Pclm.Core.Storage;

namespace Pclm.App;

/// <summary>
/// 떠 있는 작업자료 창을 앞으로 불러낸다(<see cref="Home.ActivationName"/>).
///
/// <para><b>무엇이 막혀 있었나.</b> 창은 홈마다 하나라(ADR-030) 두 번째로 띄우면 「이미 열려 있습니다」 만 듣고
/// 끝났다. 창이 알림 영역에 숨어 있으면 그 말만으로는 창을 찾을 길이 없다 — 내 작업자료 파일을 두 번 눌러도 같은
/// 길을 탄다. 그래서 잠금을 얻지 못한 쪽이 이름 붙은 신호를 울리고 조용히 끝나며, 떠 있는 쪽이 그 신호를 듣고 창을
/// 띄워 앞으로 낸다.</para>
///
/// <para>앞으로 내는 권리는 <b>방금 사람이 띄운 쪽</b>이 쥐고 있다 — 떠 있던 창이 스스로 앞으로 나오려 하면 Windows
/// 가 막고 작업 표시줄만 깜박인다. 울리기 전에 그 권리를 넘긴다(<c>AllowSetForegroundWindow</c>).</para>
/// </summary>
internal sealed class Activation : IDisposable
{
    private readonly EventWaitHandle _signal;
    private readonly RegisteredWaitHandle _wait;

    private Activation(EventWaitHandle signal, RegisteredWaitHandle wait)
    {
        _signal = signal;
        _wait = wait;
    }

    /// <summary>
    /// 신호를 듣기 시작한다. <paramref name="show"/> 는 스레드풀에서 불리므로 부르는 쪽이 UI 실마리로 넘긴다.
    /// 듣지 못하게 되었으면 <c>null</c> — 그러면 두 번째로 띄운 쪽이 예전처럼 「이미 열려 있습니다」 를 알린다.
    /// </summary>
    public static Activation? Listen(Home home, Action show)
    {
        try
        {
            var signal = new EventWaitHandle(false, EventResetMode.AutoReset, home.ActivationName);
            var wait = ThreadPool.RegisterWaitForSingleObject(signal, (_, _) => show(), null, Timeout.Infinite, executeOnlyOnce: false);
            return new Activation(signal, wait);
        }
        catch (Exception e) when (e is System.IO.IOException or UnauthorizedAccessException or WaitHandleCannotBeOpenedException)
        {
            return null;
        }
    }

    /// <summary>떠 있는 창을 불러낸다. 듣는 창이 없으면(아직 시작 화면이거나 내려가는 중) <c>false</c>.</summary>
    public static bool Signal(Home home)
    {
        try
        {
            if (!EventWaitHandle.TryOpenExisting(home.ActivationName, out var signal)) return false;
            using (signal)
            {
                AllowSetForegroundWindow(ASFW_ANY);
                return signal.Set();
            }
        }
        catch (Exception e) when (e is System.IO.IOException or UnauthorizedAccessException or WaitHandleCannotBeOpenedException)
        {
            return false;
        }
    }

    public void Dispose()
    {
        _wait.Unregister(null);
        _signal.Dispose();
    }

    private const int ASFW_ANY = -1;

    [DllImport("user32.dll")]
    private static extern bool AllowSetForegroundWindow(int processId);
}

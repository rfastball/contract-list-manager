using System.ComponentModel;
using System.IO;
using System.Windows;
using Microsoft.Win32;
using Pclm.Core.Storage;
using Pclm.Core.Erp;

namespace Pclm.App;

public partial class App : Application
{
    /// <summary>홈 잠금. 쥐고만 있으면 된다 — 놓치면 손잡이가 거둬져 두 번째 창이 들어온다.</summary>
    private IDisposable? _lock;

    /// <summary>오류 기록 자리. 홈이 정해지기 전에 터지면 업무 홈에 남긴다.</summary>
    private static string _errorLog = Home.Default.ErrorLogPath;

    /// <summary>
    /// 다시 띄울 때 넘길 인자. 같은 홈·같은 화면 자리로 다시 서야 한다 — 시험 홈의 창이 바꾼 뒤 업무 홈으로
    /// 다시 서면 격리가 아니다. <c>--view</c> 는 넘기지 않는다: 바꾸는 것은 본 창뿐이다. <c>--background</c> 도
    /// 넘기지 않는다: 사람이 보던 창에서 작업자료를 바꿨으니 다시 선 창도 보여야 한다.
    /// </summary>
    private readonly List<string> _restartArgs = [];

    /// <summary>작업자료를 바꿔 다시 띄우기로 했는가. 앱이 내려간 뒤 <see cref="StartAgainIfAsked"/> 가 본다.</summary>
    private bool _restartAsked;

    /// <summary>
    /// 정말로 내리는 중인가. 알림 영역에 두기가 켜져 있으면 창의 닫기는 숨기기로 바뀌는데, 그 길이 진짜 끝내기까지
    /// 삼키면 안 된다 — 트레이의 「끝내기」·작업자료를 바꾼 뒤의 다시 띄우기·Windows 로그오프·뜻밖의 오류.
    /// 그 넷은 내리기 전에 이것을 세운다(<see cref="Quit"/>).
    /// </summary>
    private bool _exiting;

    /// <summary>작업자료 창. 열람 창에는 알림 영역도 불러내기도 없다 — 닫으면 그냥 끝난다.</summary>
    private MainWindow? _main;

    /// <summary>작업자료 창의 홈. 창의 몸가짐(<c>window.json</c>)이 여기 있다.</summary>
    private Home? _home;

    /// <summary>알림 영역의 아이콘. 「알림 영역에 두기」 가 켜져 있는 동안만 선다.</summary>
    private TrayIcon? _tray;

    /// <summary>두 번째로 띄운 쪽이 울리는 신호를 듣는다.</summary>
    private Activation? _activation;

    /// <summary>최소화를 풀 때 돌아갈 꼴. 최대화해 두고 내렸던 창은 최대화로 돌아와야 한다.</summary>
    private WindowState _restoreState = WindowState.Normal;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // 콘솔이 없는 창 프로그램이라, 터지면 까닭을 볼 데가 없다. 화면에 띄우고 파일로도 남긴다.
        DispatcherUnhandledException += (_, args) =>
        {
            Report(args.Exception);
            args.Handled = true;
            Quit(1);
        };

        // 로그오프·종료. 창이 알림 영역으로 숨으며 Windows 를 붙들지 않게 한다.
        SessionEnding += (_, _) => _exiting = true;

        // 어느 홈을 볼지 정한다. 홈 하나에 쪽지 하나 — 창·명령줄·확장 호스트가 같은 것을 본다.
        // --home 은 개발·시험 격리의 문이다: 실제 자료를 건드리지 않고 창을 띄워 보려면 딴 홈을 짚는다.
        // --erp-dev 는 개발 호스트가 보는 홈(Home.Development)의 줄임이다.
        // --db 는 걷었다. 모르는 깃발로 흘려보내면 그 사람은 시험 자료를 연 줄 알고 실제 자료를 고친다.
        if (e.Args.Contains("--db"))
            throw new InvalidOperationException("--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 띄우세요.");
        var homeArg = Option(e.Args, "--home");
        var erpDev = e.Args.Contains("--erp-dev");
        if (e.Args.Contains("--home") && homeArg is null)
            throw new InvalidOperationException("--home 뒤에 폴더를 적으세요.");
        if (erpDev && homeArg is not null)
            throw new InvalidOperationException("--erp-dev 는 개발 홈을 씁니다. --home 과 함께 쓸 수 없습니다.");
        var home = erpDev ? Home.Development : homeArg is not null ? new Home(homeArg) : Home.Default;
        _errorLog = home.ErrorLogPath;
        if (erpDev) _restartArgs.Add("--erp-dev");
        if (homeArg is not null) _restartArgs.AddRange(["--home", homeArg]);
        if (Option(e.Args, "--ui") is { } ui) _restartArgs.AddRange(["--ui", ui]);

        // 지난번에 죽은 열람 창이 남긴 사본을 걷는다. 오래된 것만 — 지금 떠 있는 열람 창의 것은 둔다.
        home.PruneViews();

        // 브라우저가 켜진 동안 확장 호스트가 붙든 exe 는 덮어쓰지 못해 이름을 바꿔 비킨다(ADR-035). 그 옛 exe 를
        // 이 exe 옆과 홈에서 치운다. 아직 그 호스트가 쥐고 있으면 지우지 못하고 다음 번으로 미룬다.
        OldExecutables.SweepAround(Environment.ProcessPath, home.Directory);

        // .pclm 을 두 번 누르면 이 exe 가 뜨게 적는다. 열람 창으로 떠도 적는다 — 받은 사람이 처음 켜는 길이
        // 「연결 프로그램」 으로 이 exe 를 찾아 고르는 것일 수 있다.
        if (home.IsDefault) FileAssociation.Refresh();

        // 열어 볼 파일. --view <파일> 이거나, 첫 인자로 놓인 파일 하나다(파일 연결이 그렇게 부른다).
        // 확장자는 보지 않는다 — 메일을 지나며 이름이 바뀐 것을 「연결 프로그램」 으로 고르면 그대로 와야 하고,
        // PCLM 파일인지는 여는 쪽이 표지로 가린다(ADR-031). 흘려보내면 사람은 고른 파일 대신 제 작업자료를 본다.
        // 내 작업자료를 짚었으면 열람이 아니라 보통으로 연다 — 사본을 떠서 읽기만 하게 하면 사람은
        // 왜 고쳐지지 않는지 모른다.
        var viewArg = Option(e.Args, "--view");
        if (e.Args.Contains("--view") && viewArg is null)
            throw new InvalidOperationException("--view 뒤에 열어 볼 파일을 적으세요.");
        if (viewArg is null && e.Args.FirstOrDefault() is { } first &&
            !first.StartsWith("--", StringComparison.Ordinal) && File.Exists(first))
            viewArg = first;
        if (viewArg is not null && !home.IsActiveWorkfile(viewArg))
        {
            StartViewer(home, viewArg, Option(e.Args, "--ui"));
            return;
        }

        // 작업자료는 한 창만 연다(ADR-030). 두 창이 같은 자료를 고치면 한쪽의 화면이 낡은 채로 덮어쓴다.
        // 작업자료를 바꾼 창이 띄운 새 창(--restarted)만 잠깐 기다린다 — 옛 창이 내려가며 잠금을 놓는 것과
        // 겹칠 수 있다. 사람이 두 번 띄운 것은 기다리지 않고 곧바로 알린다.
        var restarted = e.Args.Contains("--restarted");
        // 로그인할 때 Windows 가 켠 것(Autostart). 사람이 띄운 것이 아니라 창을 앞에 내지 않는다.
        var background = e.Args.Contains(Autostart.BackgroundFlag);
        _lock = home.TryLock(restarted ? TimeSpan.FromSeconds(5) : default);
        if (_lock is null)
        {
            // 로그인할 때 켜진 것이 이미 떠 있는 창을 불러내면 사람은 띄운 적 없는 창을 본다. 말도 없이 끝난다.
            if (background)
            {
                Shutdown(0);
                return;
            }

            // 떠 있는 창을 앞으로 불러내고 조용히 끝난다 — 알림 영역에 숨은 창이면 「이미 열려 있다」 는 말만으로는
            // 찾을 길이 없다. 다시 띄우던 길은 부르지 않는다: 그 신호는 내려가는 옛 창이 받을 수 있다.
            if (!restarted && Activation.Signal(home))
            {
                Shutdown(0);
                return;
            }

            MessageBox.Show("계약 목록이 이미 열려 있습니다.", "계약 목록", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown(0);
            return;
        }

        // 시작 화면은 본 창보다 먼저 뜬다. 창이 하나도 없을 때 그것이 닫히면 WPF 가 앱을 내리므로,
        // 본 창이 설 때까지는 손으로 내린다.
        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        // 쪽지 → 작업자료. 열 수 없으면 시작 화면이 사람에게 고르게 한다 — 여기서 빈 자료를 세우지 않는다.
        // 사람이 파일을 찾았거나 새로 만들었으면 쪽지가 바뀌었으니 처음부터 다시 본다.
        HomeState.Ready ready;
        while (true)
        {
            var state = home.Resolve();
            if (state is HomeState.Ready r) { ready = r; break; }

            if (new StartWindow(home, (HomeState.Problem)state).ShowDialog() != true)
            {
                Shutdown(0);
                return;
            }
        }

        var database = ready.Database;

        // 옛 자료를 옮겨 왔거나 판올림 전에 백업을 떴으면 조용히 넘어가지 않는다 — 어느 자료를 여는지가 먼저다.
        foreach (var note in ready.Notes)
            MessageBox.Show(note, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Information);

        // 건이 합쳐지며 사람이 이어 둔 접수나 링크가 밀렸으면 알린다. 오류를 내지 않는 자리라
        // 여기서 말하지 않으면 이어 둔 사람은 사라진 줄도 모른다.
        if (ready.NoticeGroupText is { } 건알림)
            MessageBox.Show(건알림, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Warning);

        // 확장 연결은 업무 홈에서만 고친다. 레지스트리의 호스트 연결은 컴퓨터에 하나뿐이다.
        if (home.IsDefault) ExtensionSetup.Refresh();

        // 로그인할 때 켜기가 걸려 있으면 지금 exe 자리로 고쳐 적는다 — 새 판으로 바꾸거나 옮긴 뒤 옛 자리를 가리키면
        // 다음 로그인에 아무 말 없이 켜지지 않는다. 같은 까닭으로 업무 홈에서만.
        if (home.IsDefault) Autostart.Refresh();

        // --ui 는 화면을 지어 둔 wwwroot 대신 딴 곳에서 연다. 개발 중 Vite 서버를 짚으면
        // 화면만 갈아 끼우면서 다리와 DB 는 진짜를 쓴다(run.ps1 -Dev).
        var uiOrigin = Option(e.Args, "--ui");
        var bridge = Wire(new Bridge(database, home) { IsDefaultHome = home.IsDefault, UiOrigin = uiOrigin });

        // 작업자료를 바꾼 뒤 다리가 답을 보내고 부른다. 곧바로 내리지 않고 차례를 미룬다 — 화면의 메시지를
        // 받는 처리기 안에서 WebView2 를 닫지 않게, 보낸 답이 화면에 닿을 틈을 두게.
        bridge.Restart = () => Dispatcher.BeginInvoke(() =>
        {
            _restartAsked = true;
            Quit(0);
        });

        // 창의 몸가짐은 바꾸는 그 자리에서 선다 — 다시 띄운 뒤에야 아이콘이 서면 사람은 켠 것이 안 먹은 줄 안다.
        bridge.ApplyCloseToTray = on => Dispatcher.Invoke(() => SetTray(on));

        var window = new MainWindow(bridge, uiOrigin, home.WebView2Directory);
        if (erpDev) window.Title = "계약 목록 — ERP 개발 DB";
        else if (!home.IsDefault) window.Title = $"계약 목록 — {home.Directory}";
        MainWindow = window;
        ShutdownMode = ShutdownMode.OnLastWindowClose;

        _main = window;
        _home = home;
        window.Closing += HideToTray;
        window.StateChanged += (_, _) =>
        {
            if (window.WindowState != WindowState.Minimized) _restoreState = window.WindowState;
        };
        var closeToTray = WindowPrefs.Read(home.WindowPrefsPath).CloseToTray;
        SetTray(closeToTray);
        _activation = Activation.Listen(home, () => Dispatcher.BeginInvoke(ShowMain));

        // 로그인할 때 켜졌으면 앞에 나서지 않는다. 알림 영역에 두기가 켜져 있으면 창 없이 아이콘만 선다 — 창은
        // 지어 두고 띄우지만 않으며, 화면(WebView2)은 처음 보일 때 선다. 꺼져 있으면 숨을 곳이 없으니 작업 표시줄에
        // 내려 둔다.
        if (background && closeToTray) return;
        if (background)
        {
            window.ShowActivated = false;
            window.WindowState = WindowState.Minimized;
        }
        window.Show();
        window.ShowActivated = true;
    }

    /// <summary>정말로 내린다. 닫기를 숨기기로 바꾸는 길이 이것만은 삼키지 않는다.</summary>
    private void Quit(int code)
    {
        _exiting = true;
        Shutdown(code);
    }

    /// <summary>
    /// 알림 영역 아이콘을 세우거나 거둔다. 켜져 있는 동안은 창이 보여도 아이콘이 서 있다 — 닫으면 어디로 가는지
    /// 미리 보이게.
    /// </summary>
    private void SetTray(bool on)
    {
        if (on == _tray is not null) return;
        if (!on)
        {
            _tray!.Dispose();
            _tray = null;
            return;
        }

        _tray = new TrayIcon(_main?.Title ?? "계약 목록", ShowMain, () => Quit(0));
    }

    /// <summary>
    /// 닫기(X)를 숨기기로 바꾼다. 알림 영역에 두기가 켜져 있고 정말 내리는 중이 아닐 때만.
    /// 처음 숨을 때 한 번만 풍선으로 어디로 갔는지 알린다 — 말없이 사라지면 사람은 끝난 줄 안다.
    /// </summary>
    private void HideToTray(object? sender, CancelEventArgs e)
    {
        if (_exiting || _tray is null || _main is null || _home is null) return;

        e.Cancel = true;
        _main.Hide();

        var prefs = WindowPrefs.Read(_home.WindowPrefsPath);
        if (prefs.TrayNoticeShown) return;
        _tray.Notify("계약 목록",
            "계약 목록은 알림 영역에서 계속 돕니다. 아이콘을 두 번 누르면 다시 열리고, 끝내려면 오른쪽 단추로 「끝내기」 를 고르세요.");
        try { (prefs with { TrayNoticeShown = true }).Write(_home.WindowPrefsPath); }
        // 적지 못하면 다음에 한 번 더 알릴 뿐이다.
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
    }

    /// <summary>작업자료 창을 띄워 앞으로 낸다 — 숨었으면 보이고, 최소화했으면 풀고.</summary>
    private void ShowMain()
    {
        if (_main is not { } window || _exiting) return;

        if (!window.IsVisible) window.Show();
        if (window.WindowState == WindowState.Minimized) window.WindowState = _restoreState;
        window.Activate();
    }

    /// <summary>
    /// 잠금을 놓는다. 다시 띄우는 길이면 풀에 남은 연결도 닫는다 — 옮긴 뒤의 옛 파일을 새 창이 지우려면
    /// 이 프로세스가 그 파일을 쥐고 있지 않아야 한다.
    /// </summary>
    protected override void OnExit(ExitEventArgs e)
    {
        if (_restartAsked) Microsoft.Data.Sqlite.SqliteConnection.ClearAllPools();
        _activation?.Dispose();
        _activation = null;
        _tray?.Dispose();
        _tray = null;
        _lock?.Dispose();
        _lock = null;
        base.OnExit(e);
    }

    /// <summary>
    /// 작업자료를 바꿨으면 같은 인자로 다시 띄운다(ADR-032 의 5). 앱이 다 내려간 뒤 <see cref="Program"/> 이 부른다 —
    /// 창·다리·연결·잠금을 모두 놓은 다음이어야 새 창이 잠금과 파일을 곧바로 얻는다.
    /// </summary>
    public void StartAgainIfAsked()
    {
        if (!_restartAsked) return;

        var start = new System.Diagnostics.ProcessStartInfo(
            Environment.ProcessPath ?? throw new InvalidOperationException("이 프로그램의 자리를 알 수 없습니다."))
        {
            UseShellExecute = false,
        };
        foreach (var arg in _restartArgs) start.ArgumentList.Add(arg);
        start.ArgumentList.Add("--restarted");
        System.Diagnostics.Process.Start(start)?.Dispose();
    }

    /// <summary>
    /// 열람 창(ADR-031). <b>홈 잠금을 잡지 않고, 쪽지를 읽어 작업자료를 열지 않고, 확장 연결을 고치지 않는다</b> —
    /// 이 프로세스는 활성 작업자료와 확장 호스트에 닿을 길이 없어야 한다. 그래야 작업자료 창이 떠 있는
    /// 동안에도 몇 개든 띄울 수 있다(ADR-030).
    ///
    /// <para>원본을 열지 않고 홈의 임시 사본을 연다. 창을 닫으면 사본을 지운다.</para>
    /// </summary>
    private void StartViewer(Home home, string path, string? uiOrigin)
    {
        ViewedFile viewed;
        try
        {
            viewed = PclmFile.OpenView(path, home.ViewDirectory);
        }
        catch (PclmFileException error)
        {
            MessageBox.Show(error.Message, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Warning);
            Shutdown(1);
            return;
        }

        var bridge = Wire(new Bridge(viewed.Database, home, viewed)
        {
            IsDefaultHome = home.IsDefault,
            UiOrigin = uiOrigin,
        });

        var window = new MainWindow(bridge, uiOrigin, home.WebView2Directory)
        {
            Title = $"계약 목록 — 열람: {Path.GetFileName(viewed.SourcePath)} ({PclmRole.Name(viewed.Role)})",
        };

        // 다리가 먼저 닫힌다(MainWindow 가 생성자에서 먼저 걸었다) — 그것이 쥔 연결이 놓여야 사본이 지워진다.
        window.Closed += (_, _) => viewed.Dispose();
        MainWindow = window;
        window.Show();
    }

    /// <summary>
    /// 고르기 창을 다리에 단다. 작업자료 창과 열람 창이 같은 것을 단다 — 열람 창의 다리는 고치는 요청을
    /// 거절하므로 고르기 창이 열릴 일도 없지만, 따로 짜면 한쪽만 고쳐지는 자리가 생긴다.
    /// </summary>
    private static Bridge Wire(Bridge bridge)
    {
        // 무엇을 고르라는 것인지는 다리가 안다 — 제목도 처음 열 자리도 부르는 쪽에서 온다.
        // 지금 가리키는 폴더에서 열어 준다: 바꾸는 사람은 대개 그 근처로 옮긴다.
        bridge.PickFolder = prompt => OnUiThread(() =>
        {
            var dialog = new OpenFolderDialog { Title = prompt.Title };
            if (!string.IsNullOrEmpty(prompt.Initial) && Directory.Exists(prompt.Initial))
                dialog.InitialDirectory = prompt.Initial;

            return dialog.ShowDialog() == true ? dialog.FolderName : null;
        });

        bridge.PickPlanExcel = () => OnUiThread(() =>
        {
            var dialog = new OpenFileDialog
            {
                Title = "계획 엑셀 고르기",
                Filter = "엑셀 통합 문서 (*.xlsx)|*.xlsx",
                Multiselect = false,
            };

            return dialog.ShowDialog() == true ? dialog.FileName : null;
        });

        // 열어 볼 자료·작업자료로 쓸 자료. 받은 제출본·취합본은 대개 문서 폴더 근처에 떨어져 있다.
        bridge.PickOpenFile = title => OnUiThread(() =>
        {
            var dialog = new OpenFileDialog
            {
                Title = title,
                Filter = "계약 목록 자료 (*.pclm)|*.pclm",
                Multiselect = false,
                InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
            };

            return dialog.ShowDialog() == true ? dialog.FileName : null;
        });

        // 문서 폴더에서 연다 — 여는 자리를 정해 주지 않으면 프로그램 폴더가 걸릴 수 있고,
        // 거기에는 쓰지 못한다.
        bridge.PickSavePath = prompt => OnUiThread(() =>
        {
            var dialog = new SaveFileDialog
            {
                Title = prompt.Title,
                Filter = prompt.Filter,
                FileName = prompt.FileName,
                DefaultExt = prompt.DefaultExt,
                AddExtension = true,
                InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
            };

            return dialog.ShowDialog() == true ? dialog.FileName : null;
        });

        return bridge;
    }

    /// <summary>인자 뒤에 붙은 값. 없으면 <c>null</c>.</summary>
    private static string? Option(string[] args, string name)
    {
        var i = Array.IndexOf(args, name);
        return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
    }

    /// <summary>
    /// UI 실마리에서 실행한다. <b>고르기 창은 여기서 열어야 한다.</b>
    ///
    /// <para><b>무엇이 잘못돼 있었나.</b> 다리는 오래 걸리는 일이 화면을 멈추지 않게
    /// <c>Task.Run</c> 으로 넘기는데, 고르기 창까지 그 실마리에서 열고 있었다. 창은 뜬다 —
    /// 그래서 눈에 잘 띄지 않는다. 문제는 <b>주인이 없다는 것</b>이다. 인자 없는
    /// <c>ShowDialog()</c> 는 <b>부르는 실마리의 활성 창</b>을 주인으로 삼는데, 스레드풀
    /// 실마리에는 창이 없어 엉뚱한 것이 주인이 되거나 아무도 주인이 되지 않는다.</para>
    ///
    /// <para>주인 없는 대화창은 <b>앱 창에 모달이 아니다</b> — 뒤로 숨을 수 있고, 그 사이
    /// 본 창은 그대로 눌린다. 누른 사람 눈에는 단추가 아무 일도 하지 않은 것으로 보인다.
    /// 실측(창 목록): 스레드풀에서 열면 대화창의 주인이 본 창이 아니고,
    /// UI 실마리로 넘기면 본 창의 핸들이 그대로 주인으로 박힌다.</para>
    ///
    /// <para>막히지 않는다. 다리가 <c>await</c> 로 넘긴 사이 UI 실마리는 메시지를 돌리고 있어
    /// <c>Invoke</c> 가 곧바로 처리된다.</para>
    /// </summary>
    private static T OnUiThread<T>(Func<T> work)
    {
        var dispatcher = Current.Dispatcher;
        return dispatcher.CheckAccess() ? work() : dispatcher.Invoke(work);
    }

    private static void Report(Exception exception)
    {
        var log = _errorLog;

        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(log)!);
            File.AppendAllText(log, $"{DateTime.Now:O}{Environment.NewLine}{exception}{Environment.NewLine}{Environment.NewLine}");
        }
        catch
        {
            // 기록조차 못 남기는 상황이면 화면에 띄우는 것으로 족하다.
        }

        MessageBox.Show(
            $"{exception.Message}{Environment.NewLine}{Environment.NewLine}자세한 내용: {log}",
            "계약 목록", MessageBoxButton.OK, MessageBoxImage.Error);
    }
}

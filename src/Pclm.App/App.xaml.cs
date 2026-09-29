using System.IO;
using System.Windows;
using Microsoft.Win32;
using Pclm.Core.Storage;
using Pclm.Core.Erp;

namespace Pclm.App;

public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // 콘솔이 없는 창 프로그램이라, 터지면 까닭을 볼 데가 없다. 화면에 띄우고 파일로도 남긴다.
        DispatcherUnhandledException += (_, args) =>
        {
            Report(args.Exception);
            args.Handled = true;
            Shutdown(1);
        };

        // 쌓이는 자리는 Core 가 정한다 — 명령줄도 같은 것을 본다(DataLocation.Resolve).
        // --db 는 디버그용 문이다: 실제 자료를 건드리지 않고 창을 띄워 보려면 딴 자리를 짚어야 한다.
        // Resolve 는 밀린 이사를 치르므로 어떤 연결보다도 먼저 부른다.
        var overridden = Option(e.Args, "--db");
        var erpDev = e.Args.Contains("--erp-dev");
        if (erpDev && overridden is not null)
            throw new InvalidOperationException("--erp-dev는 격리된 개발 DB만 사용합니다. --db와 함께 쓸 수 없습니다.");
        var resolved = !erpDev && overridden is null ? DataLocation.Resolve() : null;

        var database = new Database(erpDev ? ErpDevelopment.DatabasePath : overridden ?? resolved!.DbPath);
        // 판올림이 건을 다시 지으며 밀어낸 것이 있으면 여기서 받는다. 상자는 이사 알림 뒤에 띄운다 —
        // 어느 자료를 여는지가 먼저이고, 그 안에서 무엇이 밀렸는지가 다음이다.
        var 건알림 = NoticeGroupText.Render(database.Migrate());
        if (erpDev) ErpDevelopment.MarkInitialized(database);
        else if (overridden is null)
        {
            ErpConnection.Prepare(database);
            ExtensionSetup.Refresh();
        }

        // 이사했거나 이사에 실패했으면 조용히 넘어가지 않는다 — 어느 자료를 여는지가 바뀐 일이다.
        if (resolved?.Note is { } note)
            MessageBox.Show(note, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Information);

        // 건이 합쳐지며 사람이 이어 둔 접수나 링크가 밀렸으면 알린다. 오류를 내지 않는 자리라
        // 여기서 말하지 않으면 이어 둔 사람은 사라진 줄도 모른다.
        if (건알림 is not null)
            MessageBox.Show(건알림, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Warning);

        var bridge = new Bridge(database)
        {
            AllowDataMove = !erpDev && overridden is null,
            // 무엇을 고르라는 것인지는 다리가 안다 — 제목도 처음 열 자리도 부르는 쪽에서 온다.
            // 지금 가리키는 폴더에서 열어 준다: 바꾸는 사람은 대개 그 근처로 옮긴다.
            PickFolder = prompt => OnUiThread(() =>
            {
                var dialog = new OpenFolderDialog { Title = prompt.Title };
                if (!string.IsNullOrEmpty(prompt.Initial) && Directory.Exists(prompt.Initial))
                    dialog.InitialDirectory = prompt.Initial;

                return dialog.ShowDialog() == true ? dialog.FolderName : null;
            }),

            PickPlanExcel = () => OnUiThread(() =>
            {
                var dialog = new OpenFileDialog
                {
                    Title = "계획 엑셀 고르기",
                    Filter = "엑셀 통합 문서 (*.xlsx)|*.xlsx",
                    Multiselect = false,
                };

                return dialog.ShowDialog() == true ? dialog.FileName : null;
            }),

            // 문서 폴더에서 연다 — 여는 자리를 정해 주지 않으면 프로그램 폴더가 걸릴 수 있고,
            // 거기에는 쓰지 못한다.
            PickSavePath = prompt => OnUiThread(() =>
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
            }),
        };

        // --ui 는 화면을 지어 둔 wwwroot 대신 딴 곳에서 연다. 개발 중 Vite 서버를 짚으면
        // 화면만 갈아 끼우면서 다리와 DB 는 진짜를 쓴다(run.ps1 -Dev).
        var window = new MainWindow(bridge, Option(e.Args, "--ui"),
            erpDev ? Path.Combine(ErpDevelopment.DirectoryPath, "WebView2") : null);
        if (erpDev) window.Title = "계약 목록 — ERP 개발 DB";
        window.Show();
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
        var log = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            Environment.GetCommandLineArgs().Contains("--erp-dev") ? "Pclm.Erp.Dev" : "Pclm", "error.log");

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

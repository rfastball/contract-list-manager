using System.IO;
using System.Windows;
using System.Windows.Controls;
using Microsoft.Win32;
using Pclm.Core.Storage;

namespace Pclm.App;

/// <summary>
/// 작업자료를 열 수 없을 때의 시작 화면(ADR-030).
///
/// <para><b>여기서는 사람이 누르기 전에 아무것도 만들지 않는다.</b> 쪽지가 깨졌거나 가리키는 파일이 없을 때
/// 빈 자료를 세우고 넘어가면, 사람은 자료가 사라진 줄 알고 확장은 그 빈 자료에 쌓는다. 무엇이 왜 안 되는지
/// 보이고, 있는 파일을 찾을지 새로 만들지를 사람이 고른다.</para>
///
/// <para>고르고 나면 <c>DialogResult = true</c> 로 닫는다. 쪽지가 바뀌었으니 부른 쪽이 처음부터 다시 본다 —
/// 여기서 곧장 여는 길을 따로 두면 시작 흐름이 두 벌이 된다.</para>
///
/// <para>창 하나뿐이고 다시 쓸 일이 없어 XAML 없이 코드로 짓는다.</para>
/// </summary>
internal sealed class StartWindow : Window
{
    private const string Filter = "계약 목록 자료 (*.pclm)|*.pclm";

    private readonly Home _home;

    public StartWindow(Home home, HomeState.Problem problem)
    {
        _home = home;

        Title = "계약 목록";
        Width = 560;
        SizeToContent = SizeToContent.Height;
        ResizeMode = ResizeMode.NoResize;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;

        var panel = new StackPanel { Margin = new Thickness(20) };

        panel.Children.Add(new TextBlock
        {
            Text = "작업자료를 열지 못했습니다",
            FontSize = 16,
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 0, 0, 12),
        });

        panel.Children.Add(new TextBlock
        {
            Text = problem.Message,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 8),
        });

        // 경로는 고를 수 있게 둔다 — 사람은 대개 이것을 복사해 탐색기에서 찾아본다.
        if (problem.Path is { } path)
            panel.Children.Add(new TextBox
            {
                Text = path,
                IsReadOnly = true,
                BorderThickness = new Thickness(0),
                Background = System.Windows.Media.Brushes.Transparent,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 0, 0, 8),
            });

        panel.Children.Add(new TextBlock
        {
            Text = "다른 자료를 대신 열거나 빈 자료를 만들지 않았습니다. 있는 작업자료를 찾거나 새로 만드세요.",
            TextWrapping = TextWrapping.Wrap,
            Foreground = System.Windows.Media.Brushes.DimGray,
            Margin = new Thickness(0, 0, 0, 16),
        });

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
        buttons.Children.Add(Button("파일 찾기…", Find));
        buttons.Children.Add(Button("새로 만들기…", CreateNew));
        var close = Button("닫기", () => { DialogResult = false; });
        close.IsCancel = true;
        buttons.Children.Add(close);
        panel.Children.Add(buttons);

        Content = panel;
    }

    private static Button Button(string text, Action click)
    {
        var button = new Button { Content = text, MinWidth = 96, Padding = new Thickness(10, 4, 10, 4), Margin = new Thickness(8, 0, 0, 0) };
        button.Click += (_, _) => click();
        return button;
    }

    /// <summary>있는 작업자료를 고른다. 작업자료가 아니면 까닭을 보이고 이 화면에 머문다.</summary>
    private void Find()
    {
        var dialog = new OpenFileDialog { Title = "작업자료 찾기", Filter = Filter, Multiselect = false };
        if (dialog.ShowDialog(this) != true) return;

        if (Try(() => _home.Adopt(dialog.FileName))) DialogResult = true;
    }

    /// <summary>새 작업자료를 짓는다. 고른 자리에 파일이 있으면 덮지 않는다.</summary>
    private void CreateNew()
    {
        var dialog = new SaveFileDialog
        {
            Title = "새 작업자료 만들기",
            Filter = Filter,
            FileName = Home.DefaultWorkfileName,
            DefaultExt = ".pclm",
            AddExtension = true,
            // 덮는 것은 우리가 거절한다. 대화상자의 "바꾸시겠습니까?" 에 예를 눌러도 덮이지 않으니 묻지 않는다.
            OverwritePrompt = false,
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        };
        if (dialog.ShowDialog(this) != true) return;

        if (Try(() => _home.CreateNew(dialog.FileName))) DialogResult = true;
    }

    private bool Try(Action work)
    {
        try
        {
            work();
            return true;
        }
        catch (Exception e) when (e is InvalidOperationException or IOException or UnauthorizedAccessException)
        {
            MessageBox.Show(this, e.Message, "계약 목록", MessageBoxButton.OK, MessageBoxImage.Warning);
            return false;
        }
    }
}

using Forms = System.Windows.Forms;
using Drawing = System.Drawing;

namespace Pclm.App;

/// <summary>
/// 알림 영역의 아이콘. 「알림 영역에 두기」 가 켜져 있는 동안만 선다 — 닫기(X)를 누른 창이 여기로 숨는다.
///
/// <para>두 번 누르면 창을 다시 띄우고, 오른쪽 단추 메뉴로 「열기」·「끝내기」 를 고른다. 창을 닫아도 끝나지 않으니
/// <b>정말 끝내는 길은 여기 「끝내기」</b>다.</para>
///
/// <para><b>내릴 때 반드시 치운다.</b> 손잡이를 놓지 않고 프로세스가 끝나면 마우스를 갖다 댈 때까지 죽은 아이콘이
/// 알림 영역에 남는다.</para>
/// </summary>
internal sealed class TrayIcon : IDisposable
{
    private readonly Forms.NotifyIcon _icon;
    private readonly Forms.ContextMenuStrip _menu;

    /// <summary>exe 에서 뽑은 그림. 빌려 온 시스템 그림이면 <c>null</c> — 그것은 놓지 않는다.</summary>
    private readonly Drawing.Icon? _owned;

    /// <param name="text">마우스를 갖다 대면 보일 글. 알림 영역은 63자에서 자른다.</param>
    public TrayIcon(string text, Action open, Action exit)
    {
        // exe 에 따로 박은 그림이 없어 탐색기가 보이는 그 그림을 그대로 쓴다.
        try { _owned = Environment.ProcessPath is { } exe ? Drawing.Icon.ExtractAssociatedIcon(exe) : null; }
        catch (Exception e) when (e is ArgumentException or System.IO.IOException) { _owned = null; }

        _menu = new Forms.ContextMenuStrip();
        var openItem = _menu.Items.Add("열기", null, (_, _) => open());
        openItem.Font = new Drawing.Font(openItem.Font, Drawing.FontStyle.Bold);   // 두 번 누르기와 같은 일
        _menu.Items.Add(new Forms.ToolStripSeparator());
        _menu.Items.Add("끝내기", null, (_, _) => exit());

        _icon = new Forms.NotifyIcon
        {
            Icon = _owned ?? Drawing.SystemIcons.Application,
            Text = text.Length > 63 ? text[..62] + "…" : text,
            ContextMenuStrip = _menu,
        };
        _icon.MouseDoubleClick += (_, e) => { if (e.Button == Forms.MouseButtons.Left) open(); };
        _icon.Visible = true;
    }

    /// <summary>풍선 알림. 끄고 켠 몇 초 동안만 보인다 — 놓쳐도 아이콘은 남는다.</summary>
    public void Notify(string title, string text) =>
        _icon.ShowBalloonTip(10_000, title, text, Forms.ToolTipIcon.Info);

    public void Dispose()
    {
        _icon.Visible = false;
        _icon.Dispose();
        _menu.Dispose();
        _owned?.Dispose();
    }
}

using System.Globalization;
using System.Text;

namespace Pclm.Core.Erp;

/// <summary>
/// 확장이 붙고 끊긴 기록(ADR-035). 상시 연결 호스트가 홈의 <c>extension-log.txt</c> 에 한 줄씩 덧붙이고,
/// 앱은 오늘 것만 읽어 「연결 기록 · 오늘」 에 세운다.
///
/// <para>한 줄은 탭으로 가른 다섯 칸이다 — 시각(로컬 <c>yyyy-MM-ddTHH:mm:ss</c>) · 사건 · 브라우저 · 확장 판 · 그 앞의 판
/// (판 바뀜에만 — 오류면 그 코드, 나머지는 빈 칸). 사건은 붙음·끊김·판 바뀜에 더해 수집 흐름의 오류·다시 보내 풀림이
/// 있다(ADR-036). 읽는 쪽은 모르는 사건을 건너뛰므로 옛 앱은 새 사건을 보지 않을 뿐이다. 값은 확장이 보낸 것이라 <see cref="ExtensionStatus.Accepts"/> 를 지난 것만 적는다.
/// <b>끊긴 까닭은 적지 않는다</b> — 호스트는 포트가 닫힌 것만 안다.</para>
///
/// <para>Edge·Chrome 의 호스트가 함께 쓸 수 있다. 쓰는 쪽은 파일을 홀로 열고(읽기만 나눈다) 막히면 잠깐 기다려 다시
/// 열어, 줄이 섞이지 않는다. 스스로 줄인다 — 이레가 지난 줄과 500줄을 넘는 앞쪽을 버린다. 쓰지 못해도 연결은 막지
/// 않는다(부르는 쪽이 삼킨다).</para>
/// </summary>
public static class ExtensionLog
{
    public const string Connected = "connected";
    public const string Disconnected = "disconnected";
    /// <summary>붙을 때 그 브라우저의 판이 바로 앞 기록과 다르다.</summary>
    public const string Updated = "updated";
    /// <summary>
    /// 확장의 수집 흐름이 실패했다(ADR-036). 다섯째 칸이 그 까닭의 코드다 — 확장이 보낸 것은 코드뿐이고 자료(값·번호·건명)는
    /// 없다. 코드의 뜻은 앱이 안다.
    /// </summary>
    public const string Error = "error";
    /// <summary>결과를 확인하지 못한 저장을 같은 요청으로 다시 보내 풀었다.</summary>
    public const string Recovered = "recovered";

    /// <summary>확장이 상시 포트로 보내는 사건의 메서드.</summary>
    public const string EventMethod = "event";

    private static readonly System.Text.RegularExpressions.Regex Code = new(@"\A[a-z][a-z_]{0,31}\z");

    public const int MaxLines = 500;
    public static readonly TimeSpan Keep = TimeSpan.FromDays(7);

    /// <param name="Previous">판 바뀜이면 그 앞의 판, 오류면 그 코드. 아니면 빈 문자열.</param>
    public sealed record Entry(string At, string Event, string Browser, string Version, string Previous);

    public static string LogPath(string directory) => Path.Combine(directory, "extension-log.txt");

    private static string Stamp(DateTime at) => at.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture);

    /// <summary>붙음. 그 브라우저의 바로 앞 기록과 판이 다르면 「판 바뀜」 을 먼저 남긴다.</summary>
    public static void RecordConnected(string directory, string browser, string version, DateTime now)
    {
        if (!ExtensionStatus.Accepts(browser, version)) return;
        Write(directory, now, entries =>
        {
            var at = Stamp(now);
            var previous = entries.LastOrDefault(e => e.Browser == browser)?.Version;
            var added = new List<Entry>();
            if (previous is not null && previous != version) added.Add(new(at, Updated, browser, version, previous));
            added.Add(new(at, Connected, browser, version, ""));
            return added;
        });
    }

    /// <summary>끊김. 포트가 닫혔거나, 호스트가 제 실행 파일이 비켜진 것을 보고 스스로 떠났다.</summary>
    public static void RecordDisconnected(string directory, string browser, string version, DateTime now)
    {
        if (!ExtensionStatus.Accepts(browser, version)) return;
        Write(directory, now, _ => [new(Stamp(now), Disconnected, browser, version, "")]);
    }

    /// <summary>
    /// 수집 흐름의 사건 — 오류(<paramref name="code"/> 가 그 까닭) 또는 다시 보내 풀림. 코드는 짧은 영문 낱말만 받는다:
    /// 확장이 홈에 임의 글을 쓰지 못하게.
    /// </summary>
    /// <returns>적었는가. 모양이 틀리면 적지 않는다.</returns>
    public static bool RecordEvent(string directory, string browser, string version, string kind, string? code, DateTime now)
    {
        if (!ExtensionStatus.Accepts(browser, version)) return false;
        if (kind == Error && (code is null || !Code.IsMatch(code))) return false;
        if (kind is not (Error or Recovered)) return false;
        Write(directory, now, _ => [new(Stamp(now), kind, browser, version, kind == Error ? code! : "")]);
        return true;
    }

    /// <summary>오류 하나. <paramref name="Recovered"/> 는 그 뒤 같은 브라우저가 다음 오류 전에 다시 보내 풀었는가.</summary>
    public sealed record Problem(string At, string Browser, string Code, bool Recovered);

    /// <summary>
    /// 오늘의 오류(일어난 차례대로)와 오늘 전의 마지막 오류. 기록은 이레만 남으므로 그보다 앞의 것은 모른다.
    /// </summary>
    public static (List<Problem> Today, Problem? Earlier) Problems(string directory, DateTime now)
    {
        var entries = Read(directory, now - Keep);
        var problems = new List<Problem>();
        for (var i = 0; i < entries.Count; i++)
        {
            var e = entries[i];
            if (e.Event != Error) continue;
            var recovered = false;
            for (var j = i + 1; j < entries.Count; j++)
            {
                if (entries[j].Browser != e.Browser) continue;
                if (entries[j].Event == Error) break;
                if (entries[j].Event == Recovered) { recovered = true; break; }
            }
            problems.Add(new(e.At, e.Browser, e.Previous, recovered));
        }
        var today = Stamp(now.Date);
        return (problems.Where(p => string.CompareOrdinal(p.At, today) >= 0).ToList(),
            problems.LastOrDefault(p => string.CompareOrdinal(p.At, today) < 0));
    }

    /// <summary><paramref name="since"/> 부터의 기록, 일어난 차례대로. 없거나 못 읽으면 빈 목록.</summary>
    public static List<Entry> Read(string directory, DateTime since)
    {
        var from = Stamp(since);
        string text;
        try
        {
            using var stream = Retry(() => new FileStream(LogPath(directory), FileMode.Open, FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete));
            using var reader = new StreamReader(stream, Encoding.UTF8);
            text = reader.ReadToEnd();
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return []; }
        return Parse(text).Select(p => p.Entry).Where(e => string.CompareOrdinal(e.At, from) >= 0).ToList();
    }

    private static List<(string Line, Entry Entry)> Parse(string text)
    {
        var entries = new List<(string, Entry)>();
        foreach (var line in text.Split('\n'))
        {
            var cells = line.TrimEnd('\r').Split('\t');
            // 반쯤 쓴 줄·모르는 사건은 건너뛴다 — 다음에 쓸 때 줄이면서 걷힌다.
            if (cells.Length != 5 || cells[0].Length != 19 ||
                !DateTime.TryParseExact(cells[0], "yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture, DateTimeStyles.None, out _) ||
                cells[1] is not (Connected or Disconnected or Updated or Error or Recovered) || !ExtensionStatus.Accepts(cells[2], cells[3]))
                continue;
            entries.Add((string.Join('\t', cells), new(cells[0], cells[1], cells[2], cells[3], cells[4])));
        }
        return entries;
    }

    private static void Write(string directory, DateTime now, Func<List<Entry>, List<Entry>> add)
    {
        Directory.CreateDirectory(directory);
        using var stream = Retry(() => new FileStream(LogPath(directory), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.Read));
        string text;
        using (var reader = new StreamReader(stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: false, leaveOpen: true))
            text = reader.ReadToEnd();
        var lines = text.Split('\n').Count(l => l.TrimEnd('\r').Length > 0);
        var parsed = Parse(text);
        var added = add(parsed.Select(p => p.Entry).ToList());

        var cutoff = Stamp(now - Keep);
        var kept = parsed.Where(p => string.CompareOrdinal(p.Entry.At, cutoff) >= 0).Select(p => p.Line)
            .Concat(added.Select(Line)).ToList();
        if (kept.Count > MaxLines) kept = kept.Skip(kept.Count - MaxLines).ToList();

        var utf8 = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);
        if (kept.Count == lines + added.Count)
        {
            // 버릴 것이 없다 — 끝에 덧붙이기만 한다.
            stream.Seek(0, SeekOrigin.End);
            if (stream.Length > 0 && !text.EndsWith('\n')) stream.Write(utf8.GetBytes("\n"));
            stream.Write(utf8.GetBytes(string.Concat(added.Select(e => Line(e) + "\n"))));
        }
        else
        {
            var bytes = utf8.GetBytes(string.Concat(kept.Select(l => l + "\n")));
            stream.SetLength(0);
            stream.Write(bytes);
        }
        stream.Flush();
    }

    private static string Line(Entry e) => string.Join('\t', e.At, e.Event, e.Browser, e.Version, e.Previous);

    /// <summary>다른 호스트가 쥐고 있으면 잠깐 기다려 다시 연다. 오래 막히면 던진다 — 부르는 쪽이 삼킨다.</summary>
    private static FileStream Retry(Func<FileStream> open)
    {
        for (var attempt = 0; ; attempt++)
        {
            try { return open(); }
            catch (IOException e) when (attempt < 40 && e is not FileNotFoundException and not DirectoryNotFoundException)
            { Thread.Sleep(25); }
        }
    }
}

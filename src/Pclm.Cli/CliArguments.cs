namespace Pclm.Cli;

internal sealed class CliArguments
{
    private readonly Dictionary<string, string> _options = new(StringComparer.Ordinal);
    private readonly HashSet<string> _flags = new(StringComparer.Ordinal);
    public List<string> Positionals { get; } = [];

    public string? Option(string name) => _options.GetValueOrDefault(name);
    public bool Flag(string name) => _flags.Contains(name);

    public static CliArguments Parse(string[] args)
    {
        var parsed = new CliArguments();
        for (var i = 1; i < args.Length; i++)
        {
            var arg = args[i];
            if (arg == "--")
            {
                parsed.Positionals.AddRange(args.Skip(i + 1));
                break;
            }
            if (!arg.StartsWith("--", StringComparison.Ordinal))
            {
                parsed.Positionals.Add(arg);
                continue;
            }
            if (arg is "--계열" or "--예")
            {
                parsed._flags.Add(arg);
                continue;
            }
            if (arg == "--db")
                throw new ArgumentException("--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 여세요.");
            if (arg is not ("--home" or "--file" or "--out" or "--이름"))
                throw new ArgumentException($"모르는 옵션입니다: {arg}. -- 뒤에는 입력값을 그대로 적을 수 있습니다.");
            if (i + 1 >= args.Length || args[i + 1].StartsWith("--", StringComparison.Ordinal))
                throw new ArgumentException($"{arg} 뒤에 값을 적으세요.");
            parsed._options.TryAdd(arg, args[++i]);
        }
        return parsed;
    }
}

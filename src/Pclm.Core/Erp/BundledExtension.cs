using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Pclm.Core.Erp;

/// <summary>배포 exe 안의 확장을 브라우저가 계속 읽을 사용자 폴더에 꺼낸다.</summary>
public static class BundledExtension
{
    /// <param name="Changed">하나라도 새로 썼는가. 같은 판을 다시 준비하면 거짓이다.</param>
    public sealed record Prepared(string Folder, string Version, string ExtensionId, string ManifestPath, bool Changed);

    /// <summary>exe 에 박힌 확장의 판.</summary>
    public static string EmbeddedVersion()
    {
        using var stream = typeof(BundledExtension).Assembly.GetManifestResourceStream("extension/manifest.json")
            ?? throw new InvalidOperationException("내장 확장이 없습니다. 실행 파일을 다시 빌드하세요.");
        using var manifest = JsonDocument.Parse(stream);
        return manifest.RootElement.GetProperty("version").GetString()!;
    }

    public static Prepared Prepare(string directory, string executable)
    {
        executable = Path.GetFullPath(executable);
        if (!File.Exists(executable) || !string.Equals(Path.GetExtension(executable), ".exe", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("배포된 계약목록.exe에서 확장을 준비하세요.");

        var assembly = typeof(BundledExtension).Assembly;
        var resources = assembly.GetManifestResourceNames().Where(n => n.StartsWith("extension/", StringComparison.Ordinal)).ToArray();
        using var manifestStream = assembly.GetManifestResourceStream("extension/manifest.json")
            ?? throw new InvalidOperationException("내장 확장이 없습니다. 실행 파일을 다시 빌드하세요.");
        using var manifest = JsonDocument.Parse(manifestStream);
        var key = Convert.FromBase64String(manifest.RootElement.GetProperty("key").GetString()!);
        var id = string.Concat(SHA256.HashData(key).Take(16).SelectMany(b => new[] { (char)('a' + (b >> 4)), (char)('a' + (b & 15)) }));
        var origin = $"chrome-extension://{id}/";
        var folder = Path.Combine(Path.GetFullPath(directory), "extension");
        Directory.CreateDirectory(folder);
        var changed = false;
        // manifest를 마지막에 교체한다. 경로와 공개 키는 업데이트 뒤에도 같아야 한다.
        foreach (var resource in resources.OrderBy(n => n == "extension/manifest.json"))
        {
            using var stream = assembly.GetManifestResourceStream(resource)!;
            using var bytes = new MemoryStream();
            stream.CopyTo(bytes);
            changed |= Write(Path.Combine(folder, Path.GetFileName(resource)), bytes.ToArray());
        }
        var hostManifest = Path.Combine(directory, ErpConnection.HostName + ".json");
        changed |= Write(hostManifest, JsonSerializer.SerializeToUtf8Bytes(new
        {
            name = ErpConnection.HostName, description = "PCLM ERP", path = executable,
            type = "stdio", allowed_origins = new[] { origin },
        }));
        changed |= Write(Path.Combine(directory, "extension-origin.txt"), Encoding.UTF8.GetBytes(origin));
        return new(folder, manifest.RootElement.GetProperty("version").GetString()!, id, Path.GetFullPath(hostManifest), changed);
    }

    // 앱을 켤 때마다 불린다. 같은 것을 다시 쓰면 브라우저가 읽던 파일이 까닭 없이 바뀐다.
    private static bool Write(string path, byte[] bytes)
    {
        if (File.Exists(path) && File.ReadAllBytes(path).AsSpan().SequenceEqual(bytes)) return false;
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllBytes(temporary, bytes);
            File.Move(temporary, path, overwrite: true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
        return true;
    }
}

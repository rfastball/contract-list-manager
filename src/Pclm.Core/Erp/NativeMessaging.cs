using System.Buffers.Binary;
using System.Text.Json;

namespace Pclm.Core.Erp;

/// <summary>Chrome/Edge Native Messaging: stdout에는 길이 접두사와 JSON만 쓴다.</summary>
public static class NativeMessaging
{
    public const int MaxRequestBytes = 768 * 1024;
    public const int MaxResponseBytes = 1024 * 1024 - 1024;

    public static void Run(Stream input, Stream output, Func<JsonElement, object> dispatch)
    {
        var header = new byte[4];
        while (true)
        {
            var first = input.ReadByte();
            if (first < 0) return;
            header[0] = (byte)first;
            input.ReadExactly(header.AsSpan(1));
            var length = BinaryPrimitives.ReadUInt32LittleEndian(header);
            if (length is 0 or > MaxRequestBytes) throw new InvalidDataException("잘못된 메시지 길이");
            var payload = new byte[(int)length];
            input.ReadExactly(payload);
            object response;
            try
            {
                using var document = JsonDocument.Parse(payload, new JsonDocumentOptions { MaxDepth = 32 });
                response = dispatch(document.RootElement);
            }
            catch (JsonException)
            {
                response = new { protocolVersion = 2, requestId = (string?)null, ok = false,
                    error = new { code = "invalid_json", message = "JSON 메시지를 읽지 못했습니다." } };
            }
            var bytes = JsonSerializer.SerializeToUtf8Bytes(response, new JsonSerializerOptions(JsonSerializerDefaults.Web));
            if (bytes.Length > MaxResponseBytes) throw new InvalidDataException("응답 크기 초과");
            BinaryPrimitives.WriteUInt32LittleEndian(header, (uint)bytes.Length);
            output.Write(header);
            output.Write(bytes);
            output.Flush();
        }
    }
}

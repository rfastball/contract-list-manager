using System.Buffers.Binary;
using System.Text.Json;

namespace Pclm.Core.Erp;

/// <summary>Chrome/Edge Native Messaging: stdout에는 길이 접두사와 JSON만 쓴다.</summary>
public static class NativeMessaging
{
    public const int MaxRequestBytes = 768 * 1024;
    public const int MaxResponseBytes = 1024 * 1024 - 1024;

    /// <param name="gate">stdout 에 쓰는 것을 줄 세우는 자물쇠. 답 말고도 밀어 보내는 것(<see cref="Send"/>)이 있으면 같은
    /// 것을 넘긴다 — 두 실마리가 한 메시지의 길이와 몸을 섞어 쓰면 브라우저가 포트를 끊는다.</param>
    public static void Run(Stream input, Stream output, Func<JsonElement, object> dispatch, object? gate = null)
    {
        gate ??= new object();
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
            Send(output, response, gate);
        }
    }

    /// <summary>메시지 하나를 길이 접두와 함께 쓴다. 요청 없이 밀어 보내는 것(호스트 → 확장 명령)도 이 길로 간다.</summary>
    public static void Send(Stream output, object message, object gate)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(message, new JsonSerializerOptions(JsonSerializerDefaults.Web));
        if (bytes.Length > MaxResponseBytes) throw new InvalidDataException("응답 크기 초과");
        var header = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(header, (uint)bytes.Length);
        lock (gate)
        {
            output.Write(header);
            output.Write(bytes);
            output.Flush();
        }
    }
}

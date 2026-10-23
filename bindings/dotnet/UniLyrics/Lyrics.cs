using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Win32.SafeHandles;

namespace UniLyrics;

public sealed class LyricsException(string code, string message) : Exception($"{code}: {message}")
{
    public string Code { get; } = code;
}

public sealed class Lyrics : IDisposable
{
    internal const int RequestLimit = 1_048_576;
    internal static readonly UTF8Encoding Utf8 = new(false, true);
    internal static readonly JsonSerializerOptions Json = CreateOptions();
    private readonly object gate = new();
    private readonly NativeHandle library;
    private readonly ProcessNative process;
    private readonly FreeNative free;

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate IntPtr ProcessNative([In] byte[] input, nuint length);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void FreeNative(IntPtr response);

    private sealed class NativeHandle(IntPtr value) : SafeHandleZeroOrMinusOneIsInvalid(true)
    {
        public NativeHandle Initialize() { SetHandle(value); return this; }
        protected override bool ReleaseHandle() { NativeLibrary.Free(handle); return true; }
    }

    public Lyrics(string? libraryPath = null)
    {
        libraryPath ??= Environment.GetEnvironmentVariable("UNILYRICS_LIBRARY");
        if (libraryPath is not null && !Path.IsPathFullyQualified(libraryPath))
            throw new ArgumentException("Supply an absolute native library path.", nameof(libraryPath));
        var pointer = libraryPath is null
            ? NativeLibrary.Load("unilyrics_ffi", typeof(Lyrics).Assembly, DllImportSearchPath.SafeDirectories)
            : NativeLibrary.Load(libraryPath);
        library = new NativeHandle(pointer).Initialize();
        try
        {
            process = Marshal.GetDelegateForFunctionPointer<ProcessNative>(NativeLibrary.GetExport(pointer, "unilyrics_process"));
            free = Marshal.GetDelegateForFunctionPointer<FreeNative>(NativeLibrary.GetExport(pointer, "unilyrics_free"));
        }
        catch { library.Dispose(); throw; }
    }

    private static JsonSerializerOptions CreateOptions()
    {
        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
            UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
            MaxDepth = 128
        };
        options.Converters.Add(new StrictStringConverter());
        options.Converters.Add(new JsonStringEnumConverter<Format>(JsonNamingPolicy.SnakeCaseLower, false));
        options.Converters.Add(new JsonStringEnumConverter<Policy>(JsonNamingPolicy.SnakeCaseLower, false));
        options.Converters.Add(new JsonStringEnumConverter<Profile>(JsonNamingPolicy.SnakeCaseLower, false));
        options.Converters.Add(new JsonStringEnumConverter<ProjectionFeature>(JsonNamingPolicy.SnakeCaseLower, false));
        return options;
    }

    private sealed class StrictStringConverter : JsonConverter<string>
    {
        public override string Read(ref Utf8JsonReader reader, Type type, JsonSerializerOptions options) => reader.GetString()!;
        public override void Write(Utf8JsonWriter writer, string value, JsonSerializerOptions options)
        { Utf8.GetByteCount(value); writer.WriteStringValue(value); }
        public override void WriteAsPropertyName(Utf8JsonWriter writer, string value, JsonSerializerOptions options)
        { Utf8.GetByteCount(value); writer.WritePropertyName(value); }
    }

    internal static byte[] Encode(object request)
    {
        byte[] bytes;
        try { bytes = JsonSerializer.SerializeToUtf8Bytes(request, Json); }
        catch (EncoderFallbackException error) { throw new LyricsException("INVALID_UTF8", error.Message); }
        if (bytes.Length > RequestLimit) throw new LyricsException("INPUT_TOO_LARGE", "Request exceeds 1 MiB.");
        return bytes;
    }

    public JsonElement Process(object request)
    {
        var bytes = Encode(request);
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(library.IsClosed, this);
            bool retained = false;
            library.DangerousAddRef(ref retained);
            try
            {
                var response = process(bytes, (nuint)bytes.Length);
                if (response == IntPtr.Zero) throw new LyricsException("INVALID_RESPONSE", "Native engine returned no response.");
                try
                {
                    using var envelope = JsonDocument.Parse(Marshal.PtrToStringUTF8(response)
                        ?? throw new LyricsException("INVALID_RESPONSE", "Native response is null."));
                    var root = envelope.RootElement;
                    if (!root.GetProperty("ok").GetBoolean())
                    {
                        var error = root.GetProperty("error");
                        throw new LyricsException(error.GetProperty("code").GetString()!, error.GetProperty("message").GetString()!);
                    }
                    return root.GetProperty("result").Clone();
                }
                finally { free(response); }
            }
            finally { if (retained) library.DangerousRelease(); }
        }
    }

    internal T Call<T>(object request) => Process(request).Deserialize<T>(Json)
        ?? throw new LyricsException("INVALID_RESPONSE", "Native result is null.");
    public ParseResult Parse(string input, Format? format = null) => Call<ParseResult>(new { op = "parse", input, format });
    public Format Detect(string input) => Call<Format>(new { op = "detect", input });
    public ExportResult Export(Document document, Format format, Policy policy = Policy.Compatible) => Call<ExportResult>(new { op = "export", document, format, policy });
    public ValidationResult Validate(Document document) => Call<ValidationResult>(new { op = "validate", document });
    public Features Inspect(Document document) => Call<Features>(new { op = "inspect", document });
    public ParseResult Normalize(Document document, long offsetUs = 0) => Call<ParseResult>(new { op = "normalize", document, offset_us = offsetUs });
    public ParseResult Project(Document document, ProjectionOptions? options = null) => Call<ParseResult>(new { op = "project", document, options = options ?? new() });
    public ExportResult ExportProfile(Document document, Format format, ProjectionOptions? options = null, Policy policy = Policy.Compatible) => Call<ExportResult>(new { op = "export_profile", document, format, options = options ?? new(), policy });
    public PreservedExportResult ExportPreserved(Document document, Format format, ProjectionOptions? options = null)
    {
        var result = Call<PreservedExportResult>(new { op = "export_preserved", document, format, options = options ?? new() });
        Encode(new { op = "restore_preserved", input = result.Output, format, sidecar = result.Sidecar });
        return result;
    }
    public ParseResult RestorePreserved(string input, Format format, PreservationSidecar sidecar) => Call<ParseResult>(new { op = "restore_preserved", input, format, sidecar });
    public void Dispose() { lock (gate) library.Dispose(); }
}

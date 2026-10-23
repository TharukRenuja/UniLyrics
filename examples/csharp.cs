using UniLyrics;

using var lyrics = new Lyrics();
var parsed = lyrics.Parse("[00:01]Hello 世界 🎵", Format.Lrc);
Console.WriteLine(lyrics.Export(parsed.Document, Format.Json, Policy.Strict).Output);

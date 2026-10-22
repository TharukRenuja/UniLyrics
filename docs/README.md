# UniLyrics developer documentation

UniLyrics parses, validates and converts lyrics locally. Your application supplies the content and receives a structured document; playback, rendering and content retrieval stay with your app.

Start with [getting started](GETTING_STARTED.md), then use [installation](INSTALLATION.md) and the guide for your language.

| Guide | Purpose |
|---|---|
| [Getting started](GETTING_STARTED.md) | Parse lyrics and handle conversions |
| [Installation](INSTALLATION.md) | Build packages and configure your application |
| [Language guides](LANGUAGES.md) | APIs and examples for all ten languages |
| [Data model](DATA_MODEL.md) | Cues, segments, timing and vocal relationships |
| [API](API.md) | Operations, results and errors |
| [Player integration](PLAYER_INTEGRATION.md) | Connect lyrics to your playback clock |
| [Formats](FORMATS.md) | Supported formats and dialect boundaries |
| [Conversion behavior](COMPATIBILITY.md) | Decide which output can represent your data |
| [Platforms](PLATFORMS.md) | Requirements and distribution availability |
| [Troubleshooting](TROUBLESHOOTING.md) | Resolve loading, linking and conversion errors |

Advanced workflows: [normalization](NORMALIZATION.md), [profiles](PROFILES.md), [preservation](PRESERVATION.md) and [custom adapters](EXTENSIONS.md).

Format references: [SRT](SRT.md), [WebVTT](WEBVTT.md) and [TTML](TTML.md). The [wire contract](../spec/README.md) describes native JSON requests and responses. Complete programs are in [examples](../examples/README.md).

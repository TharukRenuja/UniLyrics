# Android integration

UniLyrics provides Java/Kotlin APIs backed by Rust and JNI. The AAR includes `arm64-v8a` and `x86_64` libraries and supports Android 10 / API 29+. It uses Android's platform JSON implementation, so do not add the desktop JSON-java dependency.

## Build the AAR

The current package builder runs on Linux. Install Rust with the Android targets, JDK 17+, Kotlin 2.4.21, Android SDK 37, Build-Tools 36.0.0 and an NDK with 16 KB-compatible native linking (the package path below uses NDK 30.0.16248370).

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo fetch --locked
python -B scripts/package_android.py \
  --ndk "$ANDROID_HOME/ndk/30.0.16248370" \
  --sdk "$ANDROID_HOME" --platform android-37.0 \
  --build-tools 36.0.0 \
  --r8-jar /absolute/path/to/r8-9.1.29.jar
```

Kotlin 2.4 requires D8/R8 9.1.29 or newer. The optional `--r8-jar` selects an already-installed standalone compiler for this package build; omit it when your Build-Tools compiler is compatible. It does not override AGP's embedded D8/R8 in consuming apps. See [Android's Kotlin tooling compatibility](https://developer.android.com/build/kotlin-support).

The builder produces the AAR, dependency JARs and a POM in `target/packages-android`. It builds both ABIs, checks native ELF alignment and compiles the packaged classes to DEX. It neither installs toolchains nor publishes packages.

## Add it to your app

For local file-based integration, add the AAR and companion Kotlin standard library/annotation JARs to your app's dependency configuration. Avoid adding a second copy of the Kotlin standard library if your build already provides the same compatible dependency. The companion POM records dependencies for repository-based distribution; Gradle file dependencies do not resolve a POM automatically.

```kotlin
dependencies {
    implementation(files("libs/unilyrics-android-0.1.0.aar"))
    implementation("org.jetbrains.kotlin:kotlin-stdlib:2.4.21")
    implementation("org.jetbrains:annotations:13.0")
}
```

Use Java 17 compilation and compatible Android desugaring tooling. Keep the AAR consumer ProGuard rules enabled when shrinking: the JNI bridge depends on native method names. Include only the ABIs your app supports, and preserve native-library packaging alignment for 16 KB devices.

## Load and parse

```java
import org.tharukrj.unilyrics.UniLyrics;
UniLyrics.loadSystem();
var parsed = UniLyrics.parse("[00:01.25]Hello", "lrc");
```

```kotlin
import org.tharukrj.unilyrics.Lyrics
Lyrics.loadSystem()
val parsed = Lyrics.parse("[00:01.25]Hello", "lrc")
```

Initialize once before SDK calls. `loadSystem()` loads the Rust engine followed by the JNI bridge from the app's packaged native libraries. The typed records and map-based operations follow the [Java/Kotlin language guide](../../docs/LANGUAGES.md).

Use the [Gradle example](../../examples/android/README.md) for a complete app, and [player integration](../../docs/PLAYER_INTEGRATION.md) to connect the document to playback.

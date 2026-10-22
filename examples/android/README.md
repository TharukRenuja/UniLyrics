# Android example

This minimal Gradle application calls UniLyrics through Java and Kotlin and displays the parsed text. It consumes a locally built AAR, uses API 29+ and targets API 37. The example handles system-bar insets, retains JNI consumer rules and enables R8 in release builds.

## Prerequisites

Use JDK 17+, a compatible Gradle 9 installation, Android SDK 37 and Build-Tools 36.0.0. This project declares AGP 9.3.3 and Kotlin 2.4.21. Set `ANDROID_HOME` to your SDK directory. Gradle needs network access on its first run to resolve plugins and dependencies; no Gradle wrapper is included.

Build the [Android packages](../../bindings/android/README.md) first. The example expects `unilyrics-android-0.1.0.aar`, `kotlin-stdlib-2.4.21.jar` and `annotations-13.0.jar` in `target/packages-android`.

## Build and run

From the repository root:

```sh
gradle --no-daemon -p examples/android :app:assembleDebug :app:assembleRelease
```

Add `-PunilyricsPackages=/absolute/path/to/packages-android` to use another artifact directory. Install the debug app on your chosen device:

```sh
"$ANDROID_HOME/platform-tools/adb" -s YOUR_SERIAL install -r examples/android/app/build/outputs/apk/debug/app-debug.apk
"$ANDROID_HOME/platform-tools/adb" -s YOUR_SERIAL shell am start -W -n org.tharukrj.unilyrics.example/.MainActivity
```

The screen displays `Java: Hello 世界 🎵` and `Kotlin: Hello 世界 🎵`. The sample is an SDK integration example, not a lyrics player. It requests no network permissions and retrieves no provider content. Release signing is application-managed; the project includes no production signing keys.

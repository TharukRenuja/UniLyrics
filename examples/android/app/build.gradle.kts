plugins {
    id("com.android.application")
}

android {
    namespace = "org.tharukrj.unilyrics.example"
    compileSdk = 37
    buildToolsVersion = "36.0.0"
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "org.tharukrj.unilyrics.example"
        minSdk = 29
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }
}
kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

val packages = rootProject.file(
    providers.gradleProperty("unilyricsPackages").getOrElse("../../target/packages-android")
)
val sdkInputs = listOf(
    "unilyrics-android-0.1.0.aar",
    "kotlin-stdlib-2.4.21.jar",
    "annotations-13.0.jar"
).map { packages.resolve(it) }

dependencies {
    implementation(files(sdkInputs))
}

tasks.named("preBuild") {
    doFirst {
        sdkInputs.forEach { require(it.isFile) { "Missing local SDK artifact: $it. Build the Android AAR first." } }
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.encoding = "UTF-8"
}

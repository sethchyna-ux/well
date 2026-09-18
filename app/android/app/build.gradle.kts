plugins {
    id("com.android.application")
    id("com.google.gms.google-services")
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    signingConfigs {
        create("release") {
            // Generate a debug keystore if none exists
            val keystoreFile = file("../release.keystore")
            if (!keystoreFile.exists()) {
                // keytool command to create a keystore (executed externally)
                // We'll generate it via a script later
                println("[!] release keystore not found; will generate on build.")
            }
            storeFile = keystoreFile
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }
    compileSdk = flutter.compileSdkVersion
    ndk {
        abiFilters += listOf("arm64-v8a", "x86_64")
    }
    defaultConfig {
        applicationId = "com.example.app"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }
    buildTypes {
        release {
            // Use the release signing config
            signingConfig = signingConfigs.getByName("release")
            minifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        debug {
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // Ensure assets folder includes CLI binaries per ABI
    sourceSets {
        getByName("main") {
            assets.srcDir("src/main/assets")
        }
    }
}

flutter {
    source = "../.."
}


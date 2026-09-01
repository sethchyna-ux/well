import 'package:firebase_core/firebase_core.dart' show FirebaseOptions;
import 'package:flutter/foundation.dart' show defaultTargetPlatform, TargetPlatform;

class DefaultFirebaseOptions {
  static FirebaseOptions get currentPlatform {
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        return android;
      case TargetPlatform.iOS:
        return ios;
      case TargetPlatform.macOS:
        return macos;
      default:
        throw UnsupportedError(
          'DefaultFirebaseOptions are not supported for this platform.',
        );
    }
  }

  static const FirebaseOptions android = FirebaseOptions(
    apiKey: 'AIzaSyCFiybtTz1GdtscQL9yGp6l2eRe03pCdEk',
    appId: '1:824249630334:android:ac7c6426f48967ae8f222f',
    messagingSenderId: '824249630334',
    projectId: 'well-shell',
    storageBucket: 'well-shell.firebasestorage.app',
  );

  static const FirebaseOptions ios = FirebaseOptions(
    apiKey: 'AIzaSyB9zLHB-9je4dqhucD2Pd5iZNJZNGmiTnk',
    appId: '1:824249630334:ios:7a1f8f8d68d24f348f222f',
    messagingSenderId: '824249630334',
    projectId: 'well-shell',
    storageBucket: 'well-shell.firebasestorage.app',
    iosBundleId: 'com.example.app',
  );

  static const FirebaseOptions macos = FirebaseOptions(
    apiKey: 'AIzaSyB9zLHB-9je4dqhucD2Pd5iZNJZNGmiTnk',
    appId: '1:824249630334:ios:7a1f8f8d68d24f348f222f',
    messagingSenderId: '824249630334',
    projectId: 'well-shell',
    storageBucket: 'well-shell.firebasestorage.app',
    iosBundleId: 'com.example.app',
  );
}

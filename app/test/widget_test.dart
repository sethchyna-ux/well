import 'package:flutter_test/flutter_test.dart';

import 'package:app/main.dart';

void main() {
  testWidgets('shows Firebase onboarding controls', (tester) async {
    await tester.pumpWidget(const MyApp());

    expect(find.text('Well Firebase Onboarding'), findsOneWidget);
    expect(find.text('Connect Firebase services'), findsOneWidget);
  });
}

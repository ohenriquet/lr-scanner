import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:jlr_sdd/main.dart';

void main() {
  testWidgets('o app inicia e monta o MaterialApp', (tester) async {
    await tester.pumpWidget(const JlrApp());
    await tester.pump();
    expect(find.byType(MaterialApp), findsOneWidget);
  });
}

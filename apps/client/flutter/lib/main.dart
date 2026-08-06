import 'package:flutter/material.dart';

void main() {
  runApp(const TorcaApp());
}

/// Minimal shared client shell used to validate the Flutter workspace.
class TorcaApp extends StatelessWidget {
  /// Creates the Torca application shell.
  const TorcaApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text('Torca 0.1'),
              Text('Foundation workspace'),
            ],
          ),
        ),
      ),
    );
  }
}

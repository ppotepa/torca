import 'package:flutter/material.dart';

class DiagnosticsScreen extends StatelessWidget {
  const DiagnosticsScreen({super.key});

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Diagnostics')),
        body: ListView(
          padding: const EdgeInsets.all(24),
          children: const <Widget>[
            ListTile(
              title: Text('Runtime'),
              subtitle: Text('Gateway connected'),
            ),
            ListTile(
              title: Text('Tor'),
              subtitle: Text('Not composed in memory preview'),
            ),
            ListTile(
              title: Text('Sensitive data'),
              subtitle: Text('Never included in diagnostic export'),
            ),
          ],
        ),
      );
}

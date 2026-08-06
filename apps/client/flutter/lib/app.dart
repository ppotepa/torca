import 'package:flutter/material.dart';

import 'gateway/engine_gateway.dart';
import 'screens/home_screen.dart';

class TorcaApp extends StatelessWidget {
  const TorcaApp({required this.gateway, super.key});

  final EngineGateway gateway;

  @override
  Widget build(BuildContext context) => MaterialApp(
        title: 'Torca',
        debugShowCheckedModeBanner: false,
        theme: ThemeData(
          colorSchemeSeed: Colors.blueGrey,
          useMaterial3: true,
        ),
        home: HomeScreen(gateway: gateway),
      );
}

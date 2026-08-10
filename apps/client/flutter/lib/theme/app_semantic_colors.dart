import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

typedef AppSemanticColors = TorcaSemanticColors;

extension AppSemanticTheme on BuildContext {
  AppSemanticColors get semanticColors => torcaColors;
}

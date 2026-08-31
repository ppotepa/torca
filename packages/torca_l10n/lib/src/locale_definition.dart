import 'package:flutter/widgets.dart';

@immutable
class TorcaLocaleDefinition {
  const TorcaLocaleDefinition({
    required this.locale,
    required this.nativeName,
    required this.flag,
  });

  final Locale locale;
  final String nativeName;
  final String flag;
}

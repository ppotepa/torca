import 'package:flutter/material.dart';

extension TorcaTypographyContext on BuildContext {
  TextStyle torcaCodeStyle([TextStyle? base]) =>
      (base ?? Theme.of(this).textTheme.bodyMedium ?? const TextStyle())
          .copyWith(fontFamily: 'JetBrainsMono', package: 'torca_ui');
}

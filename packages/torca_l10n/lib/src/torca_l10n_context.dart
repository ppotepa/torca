import 'package:flutter/widgets.dart';

import 'legacy_localizations.dart';

import 'generated/torca_localizations.dart';

/// Transitional API name shared by clients while individual screens move to
/// the generated `TorcaLocalizations` API.
extension TorcaStringsContext on BuildContext {
  TorcaStrings get strings => TorcaStrings.of(this);
}

extension TorcaGeneratedL10nContext on BuildContext {
  TorcaLocalizations get l10n => TorcaLocalizations.of(this)!;
}

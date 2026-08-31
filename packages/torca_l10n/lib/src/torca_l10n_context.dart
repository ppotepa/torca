import 'package:flutter/widgets.dart';

import 'generated/torca_localizations.dart';
import 'generated/torca_localizations_en.dart';

extension TorcaGeneratedL10nContext on BuildContext {
  TorcaLocalizations get l10n =>
      TorcaLocalizations.of(this) ?? TorcaLocalizationsEn();
}

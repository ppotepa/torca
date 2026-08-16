/// User-facing power policy.  The Rust runtime remains the source of truth;
/// these values are only the typed Flutter representation of the wire enum.
enum TorcaBatteryMode {
  automatic('automatic'),
  alwaysAvailable('always_available'),
  balanced('balanced'),
  batterySaver('battery_saver');

  const TorcaBatteryMode(this.wireValue);
  final String wireValue;

  static TorcaBatteryMode parse(String? value) => values.firstWhere(
    (candidate) => candidate.wireValue == value,
    orElse: () => TorcaBatteryMode.automatic,
  );
}

enum TorcaBackgroundSyncCadence {
  instant('instant'),
  fifteenMinutes('fifteen_minutes'),
  thirtyMinutes('thirty_minutes'),
  hourly('hourly'),
  twoHours('two_hours'),
  onOpen('on_open');

  const TorcaBackgroundSyncCadence(this.wireValue);
  final String wireValue;

  static TorcaBackgroundSyncCadence parse(String? value) => values.firstWhere(
    (candidate) => candidate.wireValue == value,
    orElse: () => TorcaBackgroundSyncCadence.instant,
  );
}

enum TorcaMeteredTransferPolicy {
  allowAll('allow_all'),
  pauseLarge('pause_large'),
  pauseAll('pause_all');

  const TorcaMeteredTransferPolicy(this.wireValue);
  final String wireValue;

  static TorcaMeteredTransferPolicy parse(String? value) => values.firstWhere(
    (candidate) => candidate.wireValue == value,
    orElse: () => TorcaMeteredTransferPolicy.pauseLarge,
  );
}

enum TorcaVisualActivityPolicy {
  full('full'),
  focusedOnly('focused_only'),
  staticOnly('static'),
  followSystem('follow_system');

  const TorcaVisualActivityPolicy(this.wireValue);
  final String wireValue;

  static TorcaVisualActivityPolicy parse(String? value) => values.firstWhere(
    (candidate) => candidate.wireValue == value,
    orElse: () => TorcaVisualActivityPolicy.followSystem,
  );
}

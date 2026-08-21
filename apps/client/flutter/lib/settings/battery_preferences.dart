/// User-facing power policy.  The Rust runtime remains the source of truth;
/// these values are only the typed Flutter representation of the wire enum.
enum TorcaBatteryMode {
  automatic('automatic'),
  alwaysAvailable('always_available'),
  batterySaver('battery_saver');

  const TorcaBatteryMode(this.wireValue);
  final String wireValue;

  static TorcaBatteryMode parse(String? value) => values.firstWhere(
    (candidate) => candidate.wireValue == value,
    // `balanced` is migrated to Automatic.  The persisted value remains
    // backwards-compatible but no longer exposes scheduling internals.
    orElse: () => TorcaBatteryMode.automatic,
  );
}

/// Compatibility value kept only so old local preferences and the generated
/// native command remain decodable.  BATTERY1 no longer exposes cadence to
/// users: RuntimeOwner uses a short background grace and durable demand.
enum TorcaBackgroundSyncCadence {
  onOpen('on_open');

  const TorcaBackgroundSyncCadence(this.wireValue);
  final String wireValue;

  static TorcaBackgroundSyncCadence parse(String? value) =>
      TorcaBackgroundSyncCadence.onOpen;
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

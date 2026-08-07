enum AppThemeMode {
  system,
  light,
  dark;

  static AppThemeMode parse(String? value) => switch (value) {
        'light' => AppThemeMode.light,
        'dark' => AppThemeMode.dark,
        _ => AppThemeMode.system,
      };

  String get storageValue => name;
}

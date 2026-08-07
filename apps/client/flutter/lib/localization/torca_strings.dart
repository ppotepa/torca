import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class TorcaStrings {
  const TorcaStrings(this.locale);

  final Locale locale;

  static const supportedLocales = <Locale>[Locale('en'), Locale('pl')];
  static const LocalizationsDelegate<TorcaStrings> delegate = _TorcaStringsDelegate();

  static TorcaStrings of(BuildContext context) =>
      Localizations.of<TorcaStrings>(context, TorcaStrings) ??
      const TorcaStrings(Locale('en'));

  bool get _pl => locale.languageCode.toLowerCase() == 'pl';

  String get settings => _pl ? 'Ustawienia' : 'Settings';
  String get appearance => _pl ? 'Wygląd' : 'Appearance';
  String get system => _pl ? 'System' : 'System';
  String get light => _pl ? 'Jasny' : 'Light';
  String get dark => _pl ? 'Ciemny' : 'Dark';
  String get language => _pl ? 'Język' : 'Language';
  String get languageSystem => _pl ? 'Język systemowy' : 'System language';
  String get languageEnglish => _pl ? 'Angielski' : 'English';
  String get languagePolish => _pl ? 'Polski' : 'Polish';
  String get privacy => _pl ? 'Prywatność' : 'Privacy';
  String get sendReadReceipts => _pl ? 'Wysyłaj potwierdzenia odczytu' : 'Send read receipts';
  String get sendReadReceiptsDescription => _pl
      ? 'Oznaczaj wiadomości lokalnie jako przeczytane, ale pozwól kontaktom zobaczyć stan Read tylko wtedy, gdy ta opcja jest włączona.'
      : 'Messages are marked read locally, but contacts see the Read state only when this option is enabled.';
  String get notifications => _pl ? 'Powiadomienia' : 'Notifications';
  String get enableNotifications => _pl ? 'Włącz powiadomienia' : 'Enable notifications';
  String get notificationPrivacy => _pl
      ? 'Pokazuj powiadomienia o prywatnych wiadomościach bez ich treści.'
      : 'Show private-message notifications without message content.';
  String get desktop => _pl ? 'Pulpit' : 'Desktop';
  String get closeToTray => _pl ? 'Zamykaj do zasobnika' : 'Close to tray';
  String get closeToTrayDescription => _pl
      ? 'Pozostaw Torca uruchomioną po zamknięciu głównego okna. Wyłącz, aby zamknięcie okna kończyło aplikację.'
      : 'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.';
  String get pairContact => _pl ? 'Połącz kontakt' : 'Pair contact';
  String get newPrivateMessage => _pl ? 'Nowa prywatna wiadomość' : 'New private message';
  String get message => _pl ? 'Wiadomość' : 'Message';
  String get reply => _pl ? 'Odpowiedź' : 'Reply';
  String get sendMessage => _pl ? 'Wyślij wiadomość' : 'Send message';
  String get attachFiles => _pl ? 'Dołącz pliki' : 'Attach files';
  String get newMessages => _pl ? 'Nowe wiadomości' : 'New messages';
  String get jumpToLatest => _pl ? 'Przejdź do najnowszej wiadomości' : 'Jump to latest message';
  String get today => _pl ? 'Dzisiaj' : 'Today';
  String get yesterday => _pl ? 'Wczoraj' : 'Yesterday';
  String get retryNow => _pl ? 'Spróbuj ponownie' : 'Retry now';
  String get retrying => _pl ? 'Ponawianie…' : 'Retrying…';
}

extension TorcaStringsContext on BuildContext {
  TorcaStrings get strings => TorcaStrings.of(this);
}

class _TorcaStringsDelegate extends LocalizationsDelegate<TorcaStrings> {
  const _TorcaStringsDelegate();

  @override
  bool isSupported(Locale locale) =>
      TorcaStrings.supportedLocales.any((item) => item.languageCode == locale.languageCode);

  @override
  Future<TorcaStrings> load(Locale locale) => SynchronousFuture(TorcaStrings(locale));

  @override
  bool shouldReload(_TorcaStringsDelegate old) => false;
}

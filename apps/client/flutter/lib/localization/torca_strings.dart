import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class TorcaStrings {
  const TorcaStrings(this.locale);

  final Locale locale;

  static const supportedLocales = <Locale>[Locale('en'), Locale('pl')];
  static const LocalizationsDelegate<TorcaStrings> delegate =
      _TorcaStringsDelegate();

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
  String get sendReadReceipts =>
      _pl ? 'Wysyłaj potwierdzenia odczytu' : 'Send read receipts';
  String get sendReadReceiptsDescription => _pl
      ? 'Oznaczaj wiadomości lokalnie jako przeczytane, ale pozwól kontaktom zobaczyć stan Read tylko wtedy, gdy ta opcja jest włączona.'
      : 'Messages are marked read locally, but contacts see the Read state only when this option is enabled.';
  String get notifications => _pl ? 'Powiadomienia' : 'Notifications';
  String get enableNotifications =>
      _pl ? 'Włącz powiadomienia' : 'Enable notifications';
  String get notificationPrivacy => _pl
      ? 'Pokazuj powiadomienia o prywatnych wiadomościach bez ich treści.'
      : 'Show private-message notifications without message content.';
  String get desktop => _pl ? 'Pulpit' : 'Desktop';
  String get closeToTray => _pl ? 'Zamykaj do zasobnika' : 'Close to tray';
  String get closeToTrayDescription => _pl
      ? 'Pozostaw Torca uruchomioną po zamknięciu głównego okna. Wyłącz, aby zamknięcie okna kończyło aplikację.'
      : 'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.';
  String get pairContact => _pl ? 'Połącz kontakt' : 'Pair contact';
  String get newPrivateMessage =>
      _pl ? 'Nowa prywatna wiadomość' : 'New private message';
  String get message => _pl ? 'Wiadomość' : 'Message';
  String get reply => _pl ? 'Odpowiedź' : 'Reply';
  String get sendMessage => _pl ? 'Wyślij wiadomość' : 'Send message';
  String get attachFiles => _pl ? 'Dołącz pliki' : 'Attach files';
  String get newMessages => _pl ? 'Nowe wiadomości' : 'New messages';
  String get jumpToLatest =>
      _pl ? 'Przejdź do najnowszej wiadomości' : 'Jump to latest message';
  String get today => _pl ? 'Dzisiaj' : 'Today';
  String get yesterday => _pl ? 'Wczoraj' : 'Yesterday';
  String get retryNow => _pl ? 'Spróbuj ponownie' : 'Retry now';
  String get retrying => _pl ? 'Ponawianie…' : 'Retrying…';
  String get blocked => _pl ? 'Zablokowany' : 'Blocked';
  String get directTorContact =>
      _pl ? 'Bezpośredni kontakt Tor' : 'Direct Tor contact';
  String get startConversation =>
      _pl ? 'Rozpocznij rozmowę' : 'Start conversation';
  String get connection => _pl ? 'Połączenie' : 'Connection';
  String get state => _pl ? 'Stan' : 'State';
  String get quality => _pl ? 'Jakość' : 'Quality';
  String get onionAddress => _pl ? 'Adres onion' : 'Onion address';
  String get connectionDetails =>
      _pl ? 'Szczegóły połączenia' : 'Connection details';
  String get contactActions => _pl ? 'Akcje kontaktu' : 'Contact actions';
  String get renameContact => _pl ? 'Zmień nazwę kontaktu' : 'Rename contact';
  String get unblockContact => _pl ? 'Odblokuj kontakt' : 'Unblock contact';
  String get blockContact => _pl ? 'Zablokuj kontakt' : 'Block contact';
  String get removeContact => _pl ? 'Usuń kontakt' : 'Remove contact';
  String get localName => _pl ? 'Nazwa lokalna' : 'Local name';
  String get cancel => _pl ? 'Anuluj' : 'Cancel';
  String get save => _pl ? 'Zapisz' : 'Save';
  String get remove => _pl ? 'Usuń' : 'Remove';
  String blockContactTitle(String name) =>
      _pl ? 'Zablokować kontakt $name?' : 'Block $name?';
  String get blockContactDescription => _pl
      ? 'Torca zamknie połączenie z tym kontaktem i nie połączy się ponownie, dopóki go nie odblokujesz.'
      : 'Torca will close the peer connection and will not reconnect until you unblock this contact.';
  String removeContactTitle(String name) =>
      _pl ? 'Usunąć kontakt $name?' : 'Remove $name?';
  String get removeContactDescription => _pl
      ? 'Usuwa to lokalną relację, historię rozmowy, oczekujące operacje i chronione dane uwierzytelniające kontaktu.'
      : 'This removes the local relationship, conversation history, pending work and protected peer credential.';
  String get couldNotRenameContact =>
      _pl ? 'Nie udało się zmienić nazwy kontaktu' : 'Could not rename contact';
  String get couldNotBlockContact =>
      _pl ? 'Nie udało się zablokować kontaktu' : 'Could not block contact';
  String get couldNotUnblockContact =>
      _pl ? 'Nie udało się odblokować kontaktu' : 'Could not unblock contact';
  String get couldNotRemoveContact =>
      _pl ? 'Nie udało się usunąć kontaktu' : 'Could not remove contact';
  String get yourIdentity => _pl ? 'Twoja tożsamość' : 'Your identity';
  String get localIdentity => _pl ? 'Tożsamość lokalna' : 'Local identity';
  String get displayName => _pl ? 'Nazwa wyświetlana' : 'Display name';
  String get torState => _pl ? 'Stan Tor' : 'Tor state';
  String get unavailable => _pl ? 'Niedostępne' : 'Unavailable';
  String get applicationMenu => _pl ? 'Menu aplikacji' : 'Application menu';
  String get newPairing => _pl ? 'Nowe parowanie' : 'New pairing';
  String get diagnostics => _pl ? 'Diagnostyka' : 'Diagnostics';
  String get aboutTorca => _pl ? 'O Torca' : 'About Torca';
  String get connectionDetailsTitle =>
      _pl ? 'Szczegoly polaczenia' : 'Connection details';
  String get contactUnavailable => _pl
      ? 'Ten kontakt nie jest juz dostepny.'
      : 'This contact is no longer available.';
  String get status => _pl ? 'Stan' : 'Status';
  String get transport => 'Transport';
  String get roundTrip => _pl ? 'Opoznienie' : 'Round trip';
  String get lastSuccessfulProbe =>
      _pl ? 'Ostatnia udana sonda' : 'Last successful probe';
  String get consecutiveFailures =>
      _pl ? 'Kolejne bledy' : 'Consecutive failures';
  String get reconnectAttempts =>
      _pl ? 'Proby ponownego polaczenia' : 'Reconnect attempts';
  String get open => _pl ? 'Otworz' : 'Open';
  String get saveAs => _pl ? 'Zapisz jako' : 'Save as';
  String get messageDetails => _pl ? 'Szczegoly wiadomosci' : 'Message details';
  String get close => _pl ? 'Zamknij' : 'Close';
  String get messageCopied => _pl ? 'Wiadomosc skopiowana' : 'Message copied';
  String get attachmentSaved => _pl ? 'Zalacznik zapisany' : 'Attachment saved';
  String get diagnosticsExported =>
      _pl ? 'Diagnostyka wyeksportowana' : 'Diagnostics exported';
  String get exportFailed => _pl ? 'Eksport nieudany' : 'Export failed';
  String get connectionSelfTest =>
      _pl ? 'Test polaczenia' : 'Connection self-test';
  String get runSelfTest => _pl ? 'Uruchom test' : 'Run self-test';
  String get exportDiagnostics =>
      _pl ? 'Eksportuj diagnostyke' : 'Export diagnostics';
  String get noMessagesYet => _pl ? 'Brak wiadomosci' : 'No messages yet';
  String get contactLabel => _pl ? 'Kontakt' : 'Contact';
}

extension TorcaStringsContext on BuildContext {
  TorcaStrings get strings => TorcaStrings.of(this);
}

class _TorcaStringsDelegate extends LocalizationsDelegate<TorcaStrings> {
  const _TorcaStringsDelegate();

  @override
  bool isSupported(Locale locale) => TorcaStrings.supportedLocales.any(
    (item) => item.languageCode == locale.languageCode,
  );

  @override
  Future<TorcaStrings> load(Locale locale) =>
      SynchronousFuture(TorcaStrings(locale));

  @override
  bool shouldReload(_TorcaStringsDelegate old) => false;
}

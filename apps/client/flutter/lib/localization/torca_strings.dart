import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import '../generated/torca_contract.dart';

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
  String get chats => _pl ? 'Rozmowy' : 'Chats';
  String get appearance => _pl ? 'Wygląd' : 'Appearance';
  String get variant => _pl ? 'Wariant' : 'Variant';
  String get batteryAvailability =>
      _pl ? 'Bateria i dostępność' : 'Battery & availability';
  String get availabilityMode => _pl ? 'Tryb dostępności' : 'Availability mode';
  String get batterySettingsDescription => _pl
      ? 'Wybierz, kiedy Torca może odroczyć pracę w tle. Przychodzące zadania nigdy nie są po cichu odrzucane.'
      : 'Choose when Torca may defer background work. Incoming work is never silently discarded.';
  String get allowDelayedBackgroundDelivery => _pl
      ? 'Zezwalaj na opóźnione dostarczanie w tle'
      : 'Allow delayed background delivery';
  String get allowDelayedBackgroundDeliveryDescription => _pl
      ? 'Wymagane, aby tryb automatyczny lub oszczędzania baterii mógł wstrzymać runtime komunikacji, gdy aplikacja jest bezczynna.'
      : 'Required before Automatic or Saver can suspend the communication runtime while the app is idle.';
  String get meteredTransfers =>
      _pl ? 'Transfery w sieci taryfowej' : 'Metered network transfers';
  String get visualActivity =>
      _pl ? 'Aktywność avatara i interfejsu' : 'Avatar and visual activity';
  String get automatic => _pl ? 'Automatycznie' : 'Automatic';
  String get alwaysAvailable => _pl ? 'Zawsze dostępny' : 'Always available';
  String get batterySaver => _pl ? 'Oszczędzanie baterii' : 'Battery saver';
  String get allowAll => _pl ? 'Zezwalaj na wszystko' : 'Allow all';
  String get pauseLarge => _pl ? 'Wstrzymuj duże pliki' : 'Pause large files';
  String get pauseAll =>
      _pl ? 'Wstrzymuj wszystkie transfery' : 'Pause all transfers';
  String get fullAnimation => _pl ? 'Pełna animacja' : 'Full animation';
  String get focusedOnly =>
      _pl ? 'Animuj tylko aktywne widoki' : 'Animate focused views';
  String get staticIdle =>
      _pl ? 'Statycznie podczas bezczynności' : 'Static when idle';
  String get followSystem =>
      _pl ? 'Zgodnie z ustawieniami systemu' : 'Follow system setting';
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
  String get audio => _pl ? 'Dźwięk' : 'Audio';
  String get microphone => _pl ? 'Mikrofon' : 'Microphone';
  String get audioOutput => _pl ? 'Wyjście audio' : 'Audio output';
  String get systemDefaultAudioDevice =>
      _pl ? 'Domyślne urządzenie systemowe' : 'System default device';
  String defaultAudioDevice(String name) =>
      _pl ? '$name (domyślne)' : '$name (default)';
  String get audioDeviceUnavailable => _pl
      ? 'Nie można użyć wybranego urządzenia audio.'
      : 'The selected audio device is unavailable.';
  String get closeToTray => _pl ? 'Zamykaj do zasobnika' : 'Close to tray';
  String get closeToTrayDescription => _pl
      ? 'Pozostaw Torca uruchomioną po zamknięciu głównego okna. Wyłącz, aby zamknięcie okna kończyło aplikację.'
      : 'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.';
  String get pairContact => _pl ? 'Połącz kontakt' : 'Pair contact';
  String get newPrivateMessage =>
      _pl ? 'Nowa prywatna wiadomość' : 'New private message';
  String get message => _pl ? 'Wiadomość' : 'Message';
  String get originalMessageUnavailable => _pl
      ? 'Oryginalna wiadomość jest niedostępna'
      : 'Original message unavailable';
  String get senderYou => _pl ? 'Ty' : 'You';
  String get senderContact => _pl ? 'Kontakt' : 'Contact';
  String get outgoingMessage =>
      _pl ? 'Wiadomość wychodząca' : 'Outgoing message';
  String get incomingMessage =>
      _pl ? 'Wiadomość przychodząca' : 'Incoming message';
  String get messageActions => _pl ? 'Akcje wiadomości' : 'Message actions';
  String get sent => _pl ? 'Wysłano' : 'Sent';
  String get delivered => _pl ? 'Dostarczono' : 'Delivered';
  String get read => _pl ? 'Odczytano' : 'Read';
  String get messageQueued => _pl
      ? 'W kolejce — oczekiwanie na bezpośrednie połączenie'
      : 'Queued — waiting for a direct peer connection';
  String get queued => _pl ? 'W kolejce' : 'Queued';
  String get deliveryFailed =>
      _pl ? 'Dostarczenie nieudane' : 'Delivery failed';
  String get bookmarkMessage => _pl ? 'Zapisz wiadomość' : 'Bookmark message';
  String get removeBookmark => _pl ? 'Usuń zakładkę' : 'Remove bookmark';
  String get reply => _pl ? 'Odpowiedź' : 'Reply';
  String get sendMessage => _pl ? 'Wyślij wiadomość' : 'Send message';
  String get attachFiles => _pl ? 'Dołącz pliki' : 'Attach files';
  String get emoji => _pl ? 'Emoji' : 'Emoji';
  String get recentEmoji => _pl ? 'Ostatnio używane' : 'Recently used';
  String get draft => _pl ? 'Szkic' : 'Draft';
  String get newMessages => _pl ? 'Nowe wiadomości' : 'New messages';
  String get jumpToLatest =>
      _pl ? 'Przejdź do najnowszej wiadomości' : 'Jump to latest message';
  String get today => _pl ? 'Dzisiaj' : 'Today';
  String get yesterday => _pl ? 'Wczoraj' : 'Yesterday';
  String get retryNow => _pl ? 'Spróbuj ponownie' : 'Retry now';
  String get retrying => _pl ? 'Ponawianie…' : 'Retrying…';
  String get blocked => _pl ? 'Zablokowany' : 'Blocked';
  String providerName(String provider) => switch (provider.toLowerCase()) {
    'iroh' => 'Iroh',
    'memory' => _pl ? 'Testowa pamięć' : 'Memory test',
    _ => provider.isEmpty ? (_pl ? 'Komunikacja' : 'Communication') : provider,
  };
  String directProviderContact(String provider) => _pl
      ? 'Bezpośredni kontakt ${providerName(provider)}'
      : 'Direct ${providerName(provider)} contact';
  String connectingPeerThrough(String provider) => _pl
      ? 'Łączenie z kontaktem przez ${providerName(provider)}'
      : 'Connecting to peer through ${providerName(provider)}';
  String reconnectingPeerThrough(String provider) => _pl
      ? 'Ponowne łączenie z kontaktem przez ${providerName(provider)}'
      : 'Reconnecting to peer through ${providerName(provider)}';
  String providerReady(String provider) => _pl
      ? '${providerName(provider)} gotowy'
      : '${providerName(provider)} ready';
  String providerStarting(String provider) => _pl
      ? '${providerName(provider)} uruchamia się'
      : '${providerName(provider)} starting';
  String providerReconnecting(String provider) => _pl
      ? '${providerName(provider)} ponownie się łączy'
      : '${providerName(provider)} reconnecting';
  String providerStateLabel(String provider, String state) =>
      '${providerName(provider)}: ${state.isEmpty ? 'offline' : state}';
  String get startConversation =>
      _pl ? 'Rozpocznij rozmowę' : 'Start conversation';
  String get connection => _pl ? 'Połączenie' : 'Connection';
  String get state => _pl ? 'Stan' : 'State';
  String get quality => _pl ? 'Jakość' : 'Quality';
  String get connectionDetails =>
      _pl ? 'Szczegóły połączenia' : 'Connection details';
  String get contactActions => _pl ? 'Akcje kontaktu' : 'Contact actions';
  String get conversationActions =>
      _pl ? 'Akcje rozmowy' : 'Conversation actions';
  String get verification => _pl ? 'Weryfikacja' : 'Verification';
  String get verified => _pl ? 'Zweryfikowano' : 'Verified';
  String get verifyContact => _pl ? 'Zweryfikuj kontakt' : 'Verify contact';
  String get resetVerification =>
      _pl ? 'Resetuj weryfikację' : 'Reset verification';
  String get unverified => _pl ? 'Niezweryfikowany' : 'Unverified';
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
  String get chooseNickname =>
      _pl ? 'Wybierz pseudonim' : 'Choose your nickname';
  String get nicknameIntro => _pl
      ? 'Bezpieczny provider komunikacji jest gotowy. Ta nazwa będzie widoczna dla kontaktów.'
      : 'The selected communication provider is ready. This name will be shown to contacts.';
  String deviceFingerprint(String fingerprint) => _pl
      ? 'Odcisk urządzenia\n$fingerprint'
      : 'Device fingerprint\n$fingerprint';
  String get nickname => _pl ? 'Pseudonim' : 'Nickname';
  String get saving => _pl ? 'Zapisywanie…' : 'Saving…';
  String get continueLabel => _pl ? 'Dalej' : 'Continue';
  String get nicknameRequired =>
      _pl ? 'Pseudonim jest wymagany' : 'Nickname is required';
  String get couldNotSaveNickname =>
      _pl ? 'Nie udało się zapisać pseudonimu' : 'Could not save nickname';
  String get couldNotRenameContact =>
      _pl ? 'Nie udało się zmienić nazwy kontaktu' : 'Could not rename contact';
  String get couldNotBlockContact =>
      _pl ? 'Nie udało się zablokować kontaktu' : 'Could not block contact';
  String get couldNotUnblockContact =>
      _pl ? 'Nie udało się odblokować kontaktu' : 'Could not unblock contact';
  String get couldNotRemoveContact =>
      _pl ? 'Nie udało się usunąć kontaktu' : 'Could not remove contact';
  String get localIdentityNotReady => _pl
      ? 'Lokalna tożsamość nie jest jeszcze gotowa'
      : 'Local identity is not ready';
  String get couldNotUpdateReaction =>
      _pl ? 'Nie udało się wysłać reakcji' : 'Could not send reaction';
  String get incompatibleStorageEpoch => _pl
      ? 'Zaszyfrowany profil lokalny jest niezgodny. Jawnie zresetuj lokalne dane Torca przed kontynuowaniem.'
      : 'The encrypted local profile is incompatible. Reset local Torca data explicitly before continuing.';
  String get profileNotReady => _pl
      ? 'Bezpieczny profil nie jest jeszcze gotowy.'
      : 'The secure profile is not ready yet.';
  String get identityChanged => _pl
      ? 'Tożsamość kontaktu uległa zmianie. Sprawdź numer bezpieczeństwa.'
      : 'The contact identity changed. Verify the Safety Number.';
  String get identityChangedSendBlocked => _pl
      ? 'Wysyłanie jest wstrzymane do czasu ponownej weryfikacji kontaktu.'
      : 'Sending is paused until this contact is verified again.';
  String get blockedSendBlocked => _pl
      ? 'Kontakt jest zablokowany. Odblokuj go, aby wysłać wiadomość.'
      : 'This contact is blocked. Unblock the contact to send a message.';
  String get pairingExpired =>
      _pl ? 'Zaproszenie wygasło.' : 'The pairing invitation has expired.';
  String get itemAlreadyExists =>
      _pl ? 'Ten element już istnieje.' : 'This item already exists.';
  String get itemNotFound => _pl
      ? 'Element nie jest już dostępny.'
      : 'The item is no longer available.';
  String get invalidInput => _pl
      ? 'Podana wartość jest nieprawidłowa.'
      : 'The supplied value is not valid.';
  String get storageFailure => _pl
      ? 'Nie udało się zakończyć operacji na szyfrowanym magazynie.'
      : 'Encrypted local storage could not complete the operation.';
  String get networkUnavailable => _pl
      ? 'Wybrane połączenie komunikacyjne jest obecnie niedostępne.'
      : 'The selected communication connection is currently unavailable.';
  String get runtimeUnavailable => _pl
      ? 'Bezpieczny runtime Torca jest obecnie niedostępny.'
      : 'The secure Torca runtime is currently unavailable.';
  String get routeRefreshRequired => _pl
      ? 'Trasa połączenia jest odświeżana. Spróbuj ponownie za chwilę.'
      : 'The communication route is being refreshed. Try again shortly.';
  String get refreshProviderRoute =>
      _pl ? 'Odśwież trasę providera' : 'Refresh provider route';
  String get routeRefreshRequested => _pl
      ? 'Zażądano odświeżenia trasy providera.'
      : 'Provider route refresh requested.';
  String get contractDecodeFailed => _pl
      ? 'Klient i runtime używają niezgodnych danych. Zbuduj i wdroż oba ponownie.'
      : 'The client and native runtime use incompatible data. Rebuild and redeploy both.';
  String get operationFailed => _pl
      ? 'Nie udało się wykonać operacji.'
      : 'The operation could not be completed.';
  String connectionQuality(String quality, String rtt) => _pl
      ? 'Jakość połączenia $quality$rtt'
      : 'Connection quality $quality$rtt';
  String get yourIdentity => _pl ? 'Twoja tożsamość' : 'Your identity';
  String get country => _pl ? 'Skad jestes?' : 'Where are you from?';
  String get unknownCountry => _pl ? 'Nie podano' : 'Unknown';
  String get polishCountry => _pl ? 'Polska' : 'Poland';
  String get englishCountry => _pl ? 'Anglia' : 'England';
  String get deleteMessage =>
      _pl ? 'Usun dla obu stron' : 'Delete for everyone';
  String get deleteMessageTitle =>
      _pl ? 'Usunac wiadomosc?' : 'Delete message?';
  String get messageDeleted => _pl ? 'Wiadomosc usunieta' : 'Message deleted';
  String get localIdentity => _pl ? 'Tożsamość lokalna' : 'Local identity';
  String get fingerprint => _pl ? 'Odcisk' : 'Fingerprint';
  String get copyFingerprint => _pl ? 'Kopiuj odcisk' : 'Copy fingerprint';
  String get fingerprintCopied =>
      _pl ? 'Odcisk skopiowany' : 'Fingerprint copied';
  String get productVersion => _pl ? 'Wersja produktu' : 'Product version';
  String get build => _pl ? 'Build' : 'Build';
  String get sourceCommit => _pl ? 'Commit źródłowy' : 'Source commit';
  String get contract => _pl ? 'Kontrakt' : 'Contract';
  String get storageEpoch => _pl ? 'Epoka magazynu' : 'Storage epoch';
  String get displayName => _pl ? 'Nazwa wyświetlana' : 'Display name';
  String get unavailable => _pl ? 'Niedostępne' : 'Unavailable';
  String get applicationMenu => _pl ? 'Menu aplikacji' : 'Application menu';
  String get newPairing => _pl ? 'Nowe parowanie' : 'New pairing';
  String get newPairingRequest =>
      _pl ? 'Nowa prośba parowania' : 'New pairing request';
  String get newDevice => _pl ? 'Nowe urządzenie' : 'New device';
  String get pairingRequestDescription => _pl
      ? 'To urządzenie dołączyło do Twojego zaproszenia. Sprawdź dane kontaktu przed akceptacją.'
      : 'This device joined your invitation. Review the contact details before accepting.';
  String get diagnostics => _pl ? 'Diagnostyka' : 'Diagnostics';
  String get aboutTorca => _pl ? 'O Torca' : 'About Torca';
  String get connectionDetailsTitle =>
      _pl ? 'Szczegoly polaczenia' : 'Connection details';
  String get contactUnavailable => _pl
      ? 'Ten kontakt nie jest juz dostepny.'
      : 'This contact is no longer available.';
  String get finalizingContact => _pl
      ? 'Finalizowanie bezpiecznego kontaktu…'
      : 'Finalizing secure contact…';
  String get newContact => _pl ? 'Nowy kontakt' : 'New contact';
  String get status => _pl ? 'Stan' : 'Status';
  String get transport => 'Transport';
  String get route => _pl ? 'Trasa providera' : 'Provider route';
  String get peerState => _pl ? 'Stan P2P' : 'P2P state';
  String connectionEvidenceNote(String provider) => _pl
      ? 'Jakość opisuje uwierzytelnione bezpośrednie połączenie z kontaktem przez ${providerName(provider)}. To dane runtime, a nie siła sygnału radiowego ani gwarancja anonimowości.'
      : 'Quality describes the authenticated direct peer link over ${providerName(provider)}. It is runtime evidence, not radio or internet signal strength.';
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
  String messageTooLong(int maximum) => _pl
      ? 'Wiadomość może mieć maksymalnie $maximum znaków.'
      : 'Messages can contain at most $maximum characters.';
  String get cancelMessage => _pl ? 'Anuluj wiadomosc' : 'Cancel message';
  String get messageCancelled =>
      _pl ? 'Wiadomosc anulowana' : 'Message cancelled';
  String get editMessage => _pl ? 'Edytuj wiadomosc' : 'Edit message';
  String get messageEdited => _pl ? 'Wiadomosc zmieniona' : 'Message edited';
  String get forwardMessage => _pl ? 'Przekaż wiadomość' : 'Forward message';
  String get reactToMessage => _pl ? 'Reakcja' : 'React';
  String get chooseConversation =>
      _pl ? 'Wybierz rozmowę' : 'Choose conversation';
  String get attachmentSaved => _pl ? 'Zalacznik zapisany' : 'Attachment saved';
  String get transfers => _pl ? 'Transfery' : 'Transfers';
  String get allOperations => _pl ? 'Wszystko' : 'All';
  String get pendingOperations => _pl ? 'Oczekujące' : 'Pending';
  String get fileTransfers => _pl ? 'Pliki' : 'Files';
  String get activeTransfers => _pl ? 'Aktywne' : 'Active';
  String get mediaTransfers => _pl ? 'Media' : 'Media';
  String get documentTransfers => _pl ? 'Dokumenty' : 'Documents';
  String get recordingTransfers => _pl ? 'Nagrania' : 'Recordings';
  String get completedTransfers => _pl ? 'Zakończone' : 'Completed';
  String get sharedMedia =>
      _pl ? 'Wspólne pliki i multimedia' : 'Shared media and files';
  String sharedMediaCount(int count) =>
      _pl ? 'Elementów: $count' : '$count item${count == 1 ? '' : 's'}';
  String get noActiveTransfers =>
      _pl ? 'Brak aktywnych transferow.' : 'No active transfers.';
  String waitingForDependency(String dependency) =>
      _pl ? 'Oczekuje: $dependency' : 'Waiting for: $dependency';
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
  String get instantMode =>
      _pl ? 'Tryb natychmiastowego polaczenia' : 'Instant mode';
  String get instantModeEnabled => _pl
      ? 'Tryb natychmiastowego polaczenia wlaczony'
      : 'Instant mode enabled';
  String get radioMode => _pl ? 'Tryb radio' : 'Radio mode';
  String get radioModeDescription => _pl
      ? 'Krotkie, maksymalnie 10-sekundowe transmisje PTT. Radio dziala dopiero, gdy obie strony je wlacza.'
      : 'Short push-to-talk transmissions of up to 10 seconds. Radio becomes available only after both contacts enable it.';
  String get radioWaitingForPeer => _pl
      ? 'Oczekiwanie, az kontakt wlaczy radio'
      : 'Waiting for the contact to enable Radio';
  String get radioConnecting => _pl
      ? 'Laczenie prywatnego kanalu audio...'
      : 'Connecting the private audio channel...';
  String get radioReady => _pl ? 'Przytrzymaj, aby mowic' : 'Hold to talk';
  String get radioRequestingFloor =>
      _pl ? 'Rezerwowanie kanalu...' : 'Requesting the channel...';
  String get radioTransmitting => _pl ? 'Nadajesz' : 'Transmitting';
  String radioReceiving(String name) =>
      _pl ? '$name nadaje' : '$name is transmitting';
  String get radioReconnecting =>
      _pl ? 'Radio laczy sie ponownie...' : 'Radio is reconnecting...';
  String get radioUnavailable => _pl
      ? 'Radio jest chwilowo niedostepne'
      : 'Radio is temporarily unavailable';
  String radioTransportFailure(String code) {
    final label = switch (code) {
      'endpoint_unavailable' =>
        _pl ? 'brak punktu koncowego' : 'endpoint unavailable',
      'connect_timeout' =>
        _pl ? 'przekroczono czas laczenia' : 'connection timeout',
      'stream_reset' => _pl ? 'strumien zostal przerwany' : 'stream reset',
      'idle_timeout' =>
        _pl ? 'kanal wygasl podczas bezczynnosci' : 'idle timeout',
      'network_changed' => _pl ? 'zmienila sie siec' : 'network changed',
      'worker_unavailable' =>
        _pl ? 'worker audio jest niedostepny' : 'audio worker unavailable',
      'protocol' => _pl ? 'blad protokolu' : 'protocol error',
      _ => _pl ? 'nieznany blad transportu' : 'unknown transport error',
    };
    return _pl ? 'Radio: $label' : 'Radio: $label';
  }

  String get microphonePermissionRequired => _pl
      ? 'Dostep do mikrofonu jest wymagany do nadawania.'
      : 'Microphone access is required to transmit.';
  String get holdToRecordVoiceClip => _pl
      ? 'Przytrzymaj, aby nagrac klip glosowy'
      : 'Hold to record a voice clip';
  String voiceClipRecording(int secondsLeft) => _pl
      ? 'Nagrywanie klipu, pozostalo $secondsLeft s'
      : 'Recording voice clip, $secondsLeft s remaining';
  String get voiceClipRecordingFailed => _pl
      ? 'Nie udalo sie nagrac klipu glosowego.'
      : 'Could not record the voice clip.';
  String get voiceMessage => _pl ? 'Wiadomosc glosowa' : 'Voice message';
  String get playVoiceMessage =>
      _pl ? 'Odtworz wiadomosc glosowa' : 'Play voice message';
  String get voiceMessageReady =>
      _pl ? 'Gotowe do odtworzenia' : 'Ready to play';
  String get voiceMessagePlayed => _pl ? 'Odtworzono' : 'Played';
  String get couldNotUpdateRadio =>
      _pl ? 'Nie udalo sie zmienic trybu radio' : 'Could not update Radio mode';
  String get couldNotStartRadio => _pl
      ? 'Nie udalo sie rozpoczac transmisji'
      : 'Could not start transmission';
  String radioEnabledBy(String actor) =>
      _pl ? '$actor wlaczyl(a) tryb radio' : '$actor enabled Radio mode';
  String radioDisabledBy(String actor) =>
      _pl ? '$actor wylaczyl(a) tryb radio' : '$actor disabled Radio mode';
  String get radioChannelReady => _pl
      ? 'Prywatny kanal radio jest gotowy'
      : 'Private Radio channel is ready';
  String get radioChannelInterrupted =>
      _pl ? 'Kanal radio zostal przerwany' : 'Radio channel was interrupted';
  String get radioChannelRestored =>
      _pl ? 'Kanal radio zostal przywrocony' : 'Radio channel was restored';

  String contactAddedToContacts(String name) =>
      _pl ? '$name dodano do kontaktów' : '$name was added to Contacts';

  String contactAcceptedJoin(String name) => _pl
      ? '$name zaakceptował(a) zaproszenie'
      : '$name accepted your join request';
  String get closeTooltip => _pl ? 'Zamknij' : 'Close';
  String buildTooltip(String build, String providerService) => _pl
      ? 'Build $build\nWersja usługi providera: $providerService'
      : 'Torca build $build\nProvider service: $providerService';
  String buildLabel(String build) => _pl ? 'build $build' : 'build $build';
  String get secureRuntimeNotReady => _pl
      ? 'Bezpieczne srodowisko nie jest gotowe'
      : 'Secure runtime is not ready';
  String get runtimePreparationFailed => _pl
      ? 'Nie udalo sie przygotowac lokalnego szyfrowanego runtime. Tozsamosc nie zostala zmieniona.'
      : 'Torca could not prepare the local encrypted runtime. Your identity has not been changed.';
  String runtimeNotReadyDiagnostic(String provider) {
    final normalized = provider.trim().toLowerCase();
    final label = switch (normalized) {
      'iroh' => 'Iroh',
      _ => provider.trim().isEmpty ? 'communication provider' : provider.trim(),
    };
    return _pl
        ? 'Runtime komunikacji ($label) nie jest gotowy. Sprawdz diagnostyke i sprobuj ponownie.'
        : 'The $label communication runtime is not ready. Check diagnostics and retry.';
  }

  String get modern => _pl ? 'Nowoczesny' : 'Modern';
  String get terminal => _pl ? 'Terminal' : 'Terminal';
  String get compactDensity => _pl ? 'Gestosc kompaktowa' : 'Compact density';
  String get comfortableDensity =>
      _pl ? 'Gestosc wygodna' : 'Comfortable density';
  String get reduceMotion => _pl ? 'Ogranicz ruch' : 'Reduce motion';
  String get rawDiagnostics => _pl ? 'Surowa diagnostyka' : 'Raw diagnostics';
  String get redactedDeveloperEventStream => _pl
      ? 'Zanonimizowany strumien zdarzen deweloperskich'
      : 'Redacted developer event stream';
  String get diagnosticsStream =>
      _pl ? 'Strumien diagnostyczny' : 'Diagnostics stream';
  String get excellent => _pl ? 'Doskonaly' : 'Excellent';
  String get good => _pl ? 'Dobry' : 'Good';
  String get fair => _pl ? 'Sredni' : 'Fair';
  String get poor => _pl ? 'Slaby' : 'Poor';
  String get unknown => _pl ? 'Nieznany' : 'Unknown';
  String get closeScanner => _pl ? 'Zamknij skaner' : 'Close scanner';
  String get generatingInvitation => _pl ? 'Generowanie…' : 'Generating…';
  String get retryGeneration => _pl ? 'Ponow generowanie' : 'Retry generation';
  String get yourInvitation => _pl ? 'Twoje zaproszenie' : 'Your invitation';
  String get joinInvitation =>
      _pl ? 'Dolacz do zaproszenia' : 'Join invitation';
  String get checkingInvitation =>
      _pl ? 'Sprawdzanie zaproszenia...' : 'Checking invitation...';
  String get invitationCode => _pl ? 'Kod zaproszenia' : 'Invitation code';
  String get enterSixCharacterCode => _pl
      ? 'Wpisz szescioznakowy kod lub zeskanuj kod QR.'
      : 'Enter a six-character code or scan the QR code.';
  String get pairingBootstrapRequired => _pl
      ? 'Dla tego providera zeskanuj kod QR albo wklej pelne zaproszenie.'
      : 'For this provider, scan the QR code or paste the full invitation link.';
  String get pairingProviderMismatch => _pl
      ? 'To zaproszenie pochodzi od innego providera komunikacji.'
      : 'This invitation belongs to a different communication provider.';
  String get invitationGenerating => _pl
      ? 'Generowanie prywatnego zaproszenia...'
      : 'Generating a private invitation...';
  String get invitationWaitingForNetwork => _pl
      ? 'Zaproszenie oczekuje na siec.'
      : 'Invitation is waiting for the network.';
  String get invitationQueued => _pl
      ? 'Zaproszenie dodane do kolejki bezpiecznej sieci.'
      : 'Invitation queued for the secure network.';
  String get invitationOperationFailed => _pl
      ? 'Operacja zaproszenia nie powiodla sie'
      : 'Invitation operation failed';
  String get providerEndpoint =>
      _pl ? 'Endpoint providera' : 'Provider endpoint';
  String get communicationProvider =>
      _pl ? 'Provider komunikacji' : 'Communication provider';
  String get communicationState =>
      _pl ? 'Stan komunikacji' : 'Communication state';
  String get endpoint => _pl ? 'Endpoint' : 'Endpoint';

  String get providerEndpointAvailable => _pl ? 'Dostępny' : 'Available';

  String get providerEndpointUnavailable => _pl ? 'Niedostępny' : 'Unavailable';
  String get invitationJoinSent => _pl
      ? 'Zadanie dolaczenia wyslane. Otrzymasz powiadomienie po akceptacji.'
      : 'Join request sent. You will be notified when it is accepted.';
  String get invitationSavedLocally => _pl
      ? 'Zapisano lokalnie. Ponowimy, gdy wybrany dostawca komunikacji bedzie gotowy.'
      : 'Saved locally. It will retry when the selected communication provider is ready.';
  String get openConversation => _pl ? 'Otworz rozmowe' : 'Open conversation';
  String get noMessagesYetDescription => _pl
      ? 'Wiadomosci sa wysylane bezposrednio przez wybrany provider komunikacji.'
      : 'Messages are sent directly through the selected communication provider.';
  String get attachmentSyncing =>
      _pl ? 'Synchronizacja zalacznika…' : 'Attachment is syncing…';
  String get closeSearch => _pl ? 'Zamknij wyszukiwanie' : 'Close search';
  String get searchConversationHint =>
      _pl ? 'Szukaj w tej rozmowie' : 'Search this conversation';
  String get typeToSearchConversation => _pl
      ? 'Wpisz tekst, aby przeszukać rozmowę.'
      : 'Type to search this conversation.';
  String get noMatchingMessages =>
      _pl ? 'Brak pasujących wiadomości.' : 'No matching messages.';
  String get preparingUpload =>
      _pl ? 'Przygotowanie wysylania' : 'Preparing upload';
  String get preparingDownload =>
      _pl ? 'Przygotowanie pobierania' : 'Preparing download';
  String get preparingSecureCopy =>
      _pl ? 'Przygotowanie bezpiecznej kopii' : 'Preparing secure copy';
  String get encrypting => _pl ? 'Szyfrowanie' : 'Encrypting';
  String get waitingToReceive =>
      _pl ? 'Oczekiwanie na odbior' : 'Waiting to receive';
  String get waitingForPeer =>
      _pl ? 'Oczekiwanie na kontakt' : 'Waiting for peer';
  String get sendingSecurely =>
      _pl ? 'Bezpieczne wysylanie' : 'Sending securely';
  String get receivingSecurely =>
      _pl ? 'Bezpieczny odbior' : 'Receiving securely';
  String get verifiedOnDevice =>
      _pl ? 'Zweryfikowano na urzadzeniu' : 'Verified on device';
  String get transferFailed => _pl ? 'Wysylanie nieudane' : 'Transfer failed';
  String get cancelled => _pl ? 'Anulowano' : 'Cancelled';
  String get attachmentAckTimeout => _pl
      ? 'oczekiwanie na potwierdzenie kontaktu'
      : 'waiting for peer acknowledgement';
  String get attachmentPeerUnavailable =>
      _pl ? 'kontakt niedostępny' : 'peer unavailable';
  String get attachmentIntegrityFailed =>
      _pl ? 'błąd integralności' : 'integrity check failed';
  String get attachmentStorageFailed =>
      _pl ? 'błąd lokalnego zapisu' : 'local storage failed';
  String get attachmentMessagePending =>
      _pl ? 'oczekiwanie na wiadomość' : 'waiting for message';
  String get attachmentDependencyMissing =>
      _pl ? 'oczekiwanie na rozmowę' : 'waiting for conversation';
  String get attachmentRetryAvailable =>
      _pl ? 'dostępna ponowna próba' : 'retry available';
  String get attachmentOperationFailed =>
      _pl ? 'Operacja zalacznika nieudana' : 'Attachment operation failed';
  String attachmentsQueued(int count) => _pl
      ? 'Dodano do kolejki: $count zalacznikow'
      : '$count ${count == 1 ? 'attachment' : 'attachments'} queued';
  String get couldNotQueueAttachment => _pl
      ? 'Nie udalo sie dodac zalacznika do kolejki'
      : 'Could not queue attachment';
  String get saveAttachment => _pl ? 'Zapisz zalacznik' : 'Save attachment';
  String get buildAndConnectionInfo =>
      _pl ? 'Informacje o buildzie i polaczeniu' : 'Build & connection info';
  String get pairContactHint => _pl
      ? 'Polacz kontakt, aby rozpoczac rozmowe.'
      : 'Pair a contact to start a conversation.';
  String get contacts => _pl ? 'Kontakty' : 'Contacts';
  String get invitations => _pl ? 'Zaproszenia' : 'Invitations';
  String get selectConversation =>
      _pl ? 'Wybierz rozmowe' : 'Select a conversation';
  String get createManageInvitations => _pl
      ? 'Tworz i zarzadzaj krotkimi, prywatnymi zaproszeniami.'
      : 'Create and manage short-lived private contact invitations.';
  String get generateInvitation =>
      _pl ? 'Wygeneruj zaproszenie' : 'Generate Invitation';
  String get copyCode => _pl ? 'Kopiuj zaproszenie' : 'Copy invitation';
  String get invitationCodeCopied =>
      _pl ? 'Pełne zaproszenie skopiowane' : 'Full invitation copied';
  String get done => _pl ? 'Gotowe' : 'Done';
  String get accept => _pl ? 'Akceptuj' : 'Accept';
  String get reject => _pl ? 'Odrzuc' : 'Reject';
  String get cancelRequest => _pl ? 'Anuluj zadanie' : 'Cancel request';
  String get cancelInvitation =>
      _pl ? 'Anuluj zaproszenie' : 'Cancel invitation';
  String get copy => _pl ? 'Kopiuj' : 'Copy';
  String get noContactsYet => _pl ? 'Brak kontaktow' : 'No contacts yet';
  String get createInvitationForContact => _pl
      ? 'Utworz zaproszenie, aby dodac prywatny kontakt.'
      : 'Create an invitation to add a private contact.';
  String contactsCount(int count) => _pl
      ? '$count kontaktow'
      : '$count private ${count == 1 ? 'contact' : 'contacts'}';
  String get openChat => _pl ? 'Otworz czat' : 'Open chat';
  String get contactInformation =>
      _pl ? 'Informacje o kontakcie' : 'Contact information';
  String get noInvitations => _pl ? 'Brak zaproszen' : 'No invitations';
  String get activeInvitationsDescription => _pl
      ? 'Aktywne zaproszenia i prosby o parowanie pojawia sie tutaj.'
      : 'Your active invitations and pairing requests will appear here.';
  String get recentInvitations =>
      _pl ? 'Ostatnie zaproszenia' : 'Recent invitations';
  String get createdInvitation =>
      _pl ? 'Utworzone zaproszenie' : 'Created invitation';
  String get joinedInvitation =>
      _pl ? 'Dolaczone zaproszenie' : 'Joined invitation';
  String pairingStateLabel(PairingState state) => switch (state) {
    PairingState.open => _pl ? 'Otwarte' : 'Open',
    PairingState.peerJoined => _pl ? 'Kontakt dolaczyl' : 'Peer joined',
    PairingState.awaitingApproval =>
      _pl ? 'Czeka na akceptacje' : 'Awaiting approval',
    PairingState.approved => _pl ? 'Zaakceptowane' : 'Approved',
    PairingState.completed => _pl ? 'Polaczone' : 'Completed',
    PairingState.rejected => _pl ? 'Odrzucone' : 'Rejected',
    PairingState.cancelled => _pl ? 'Anulowane' : 'Cancelled',
    PairingState.expired => _pl ? 'Wygasle' : 'Expired',
    PairingState.unknown => _pl ? 'Nieznany stan' : 'Unknown',
  };
  String invitationCodeLabel(String code) => _pl ? 'Kod $code' : 'Code $code';
  String get notMeasured => _pl ? 'Nie zmierzono' : 'Not measured';
  String get never => _pl ? 'Nigdy' : 'Never';
  String get presence => _pl ? 'Obecnosc' : 'Presence';
  String get lastSeen => _pl ? 'Ostatnio widziany' : 'Last seen';
  String get online => _pl ? 'Online' : 'Online';
  String lastSeenAt(String time) => _pl ? 'Ostatnio $time' : 'Last seen $time';
  String get clearConversationHistory =>
      _pl ? 'Wyczyść historię rozmowy' : 'Clear conversation history';
  String get markConversationRead =>
      _pl ? 'Oznacz jako przeczytane' : 'Mark as read';
  String get archiveConversation =>
      _pl ? 'Archiwizuj rozmowę' : 'Archive conversation';
  String get restoreConversation =>
      _pl ? 'Przywróć rozmowę' : 'Restore conversation';
  String get pinConversation => _pl ? 'Przypnij rozmowę' : 'Pin conversation';
  String get unpinConversation =>
      _pl ? 'Odepnij rozmowę' : 'Unpin conversation';
  String get muteConversation => _pl ? 'Wycisz rozmowę' : 'Mute conversation';
  String get unmuteConversation =>
      _pl ? 'Włącz powiadomienia' : 'Unmute conversation';
  String get todayUpper => _pl ? 'DZISIAJ' : 'TODAY';
  String get sampleContactName => 'Alice';
  String get sampleOnline => _pl ? 'online' : 'online';
  String get sampleTime => '14:22';
  String remoteIdentity(String? id) =>
      _pl ? 'Tozsamosc ${id ?? '-'}' : 'Identity ${id ?? '-'}';
  String get searchMessages => _pl ? 'Szukaj wiadomosci' : 'Search messages';
  String get searchChats => _pl ? 'Szukaj rozmow' : 'Search chats';
  String get clearSearch => _pl ? 'Wyczysc wyszukiwanie' : 'Clear search';
  String get noChatsMatch => _pl
      ? 'Brak rozmow pasujacych do wyszukiwania'
      : 'No chats match your search';
  String searchResultsCount(int count) =>
      _pl ? 'Wyniki: $count' : '$count ${count == 1 ? 'result' : 'results'}';
  String get refresh => _pl ? 'Odswiez' : 'Refresh';
  String get removeAttachment => _pl ? 'Usun zalacznik' : 'Remove attachment';
  String get scanQr => _pl ? 'Skanuj QR' : 'Scan QR';
  String get contactDetails => _pl ? 'Szczegoly kontaktu' : 'Contact details';
  String get contactBlocked =>
      _pl ? 'Kontakt jest zablokowany' : 'Contact is blocked';
  String get connecting => _pl ? 'Laczenie' : 'Connecting';
  String get reconnecting => _pl ? 'Ponowne laczenie' : 'Reconnecting';
  String get peerOffline => _pl ? 'Kontakt offline' : 'Peer is offline';
  String get p2pShort => 'P2P';
  String get startingShort => _pl ? 'Start' : 'Starting';
  String get reconnectingShort => _pl ? 'Laczenie' : 'Reconnecting';
  String get offlineShort => 'Offline';
  String get nativeBridge => _pl ? 'Most natywny' : 'Native bridge';
  String get localIdentityCheck => _pl ? 'Lokalna tozsamosc' : 'Local identity';
  String get directPeers => _pl ? 'Bezposrednie wezly' : 'Direct peers';
  String get noContactsPaired =>
      _pl ? 'Brak sparowanych kontaktow' : 'No contacts paired';
  String directPeerLinksReady(int ready, int total) => _pl
      ? '$ready z $total bezposrednich polaczen gotowych'
      : '$ready of $total direct peer links ready';
  String get contractSnapshotReadable =>
      _pl ? 'Snapshot kontraktu czytelny' : 'Contract snapshot readable';
  String get notInitialized => _pl ? 'Nie zainicjalizowano' : 'Not initialized';
  String get loaded => _pl ? 'Zaladowano' : 'Loaded';
  String get published => _pl ? 'Opublikowano' : 'Published';
  String get redactedHealthEventsReadable => _pl
      ? 'Zanonimizowane zdarzenia zdrowia czytelne'
      : 'Redacted health events readable';
  String get noReadableHealthEvents =>
      _pl ? 'Brak czytelnych zdarzen zdrowia' : 'No readable health events';
  String get startingSecureNetwork =>
      _pl ? 'Uruchamianie komunikacji…' : 'Starting communication…';
  String get batteryTab => _pl ? 'Bateria' : 'Battery';
  String get runtimeTab => _pl ? 'Runtime' : 'Runtime';
  String get logsTab => _pl ? 'Logi' : 'Logs';
  String get incidentTab => _pl ? 'Incydent' : 'Incident';
  String get runtimeHealth => _pl ? 'Stan runtime' : 'Runtime health';
  String get nativeLogTails => _pl ? 'Logi natywne' : 'Native log tails';
  String get nativeLogTailsDescription => _pl
      ? 'Wczytuje ograniczony, zanonimizowany fragment bieżących logów natywnych. Odczyt nie uruchamia ciągłego monitorowania.'
      : 'Loads a bounded, redacted tail from current-run native logs only. This explicit read does not keep a watcher alive.';
  String get loadCurrentRunLogs =>
      _pl ? 'Wczytaj logi bieżącego uruchomienia' : 'Load current run logs';
  String get incidentTools => _pl ? 'Narzędzia incydentu' : 'Incident tools';
  String get incidentDescription => _pl
      ? 'Uruchom autotest, oznacz bieżący stan i wyeksportuj zanonimizowany zrzut diagnostyczny. Treści wiadomości, załączniki, audio i sekrety nie są dołączane.'
      : 'Run a self-test, mark the current state and export the redacted snapshot. Message text, attachments, audio and secrets are not included.';
  String get markIncident => _pl ? 'Oznacz incydent' : 'Mark incident';
  String get observationRecording => _pl ? 'rejestrowanie' : 'recording';
  String get observationStopped => _pl ? 'zatrzymano' : 'stopped';
  String get observationWork => _pl ? 'Praca' : 'Work';
  String get regressionScore => _pl ? 'Wynik regresji' : 'Regression score';
  String get batteryObservation =>
      _pl ? 'Obserwacja baterii' : 'Battery observation';
  String get observationRecordingDescription => _pl
      ? 'Rejestrowanie zmian od punktu bazowego obserwacji.'
      : 'Recording deltas since the observation baseline.';
  String get observationStoppedDescription => _pl
      ? 'Uruchom przed scenariuszem bezczynności lub odzyskiwania, aby zapisać nową pracę.'
      : 'Start before an idle or recovery scenario to record only new work.';
  String get observationState => _pl ? 'Stan' : 'State';
  String get wakeSources => _pl ? 'Źródła wybudzeń' : 'Wake sources';
  String get startObservation =>
      _pl ? 'Rozpocznij obserwację' : 'Start observation';
  String get stopObservation =>
      _pl ? 'Zatrzymaj obserwację' : 'Stop observation';
  String get resetBaseline => _pl ? 'Zresetuj punkt bazowy' : 'Reset baseline';
  String get whyAwake => _pl ? 'Dlaczego aktywny' : 'Why awake';
  String get redactedSchedulerDescription => _pl
      ? 'Zanonimizowane wyjaśnienie harmonogramu; identyfikatory kontaktów nie są tutaj wyświetlane.'
      : 'Redacted scheduler explanation; contact identifiers are never shown here.';
  String get activeLeases => _pl ? 'Aktywne dzierżawy' : 'Active leases';
  String get activeDemands =>
      _pl ? 'Aktywne zapotrzebowania' : 'Active demands';
  String get leaseReasons => _pl ? 'Powody dzierżaw' : 'Lease reasons';
  String get scheduledWork => _pl ? 'Zaplanowana praca' : 'Scheduled work';
  String get nextDeadline => _pl ? 'Następny termin' : 'Next deadline';
  String get zeroDelayDeadlines =>
      _pl ? 'Terminy bez opóźnienia' : 'Zero-delay deadlines';
  String get identicalDeadlineReplacements => _pl
      ? 'Identyczne zastąpienia terminów'
      : 'Identical deadline replacements';
  String get exportTorcaDiagnostics =>
      _pl ? 'Eksportuj diagnostykę Torca' : 'Export Torca diagnostics';
  String get incidentSnapshotSaved => _pl
      ? 'Zrzut incydentu zapisano w lokalnej diagnostyce tego uruchomienia.'
      : 'Incident snapshot saved to this run\'s local diagnostics.';
  String get messageForwarded =>
      _pl ? 'Wiadomość przekazana' : 'Message forwarded';
  String get couldNotForwardMessage =>
      _pl ? 'Nie udało się przekazać wiadomości' : 'Could not forward message';
  String get noForwardableContent => _pl
      ? 'Ta wiadomość nie zawiera treści możliwej do przekazania.'
      : 'This message has no content that can be forwarded.';
  String forwardSkippedAttachments(int count) => _pl
      ? 'Przekazano wiadomość; pominięto $count niedostępnych załączników.'
      : 'Message forwarded; skipped $count unavailable attachment${count == 1 ? '' : 's'}.';
  String forwardNoAvailableAttachments(int count) => _pl
      ? 'Nie można przekazać: $count załącznik${count == 1 ? ' jest' : 'i są'} niedostępny${count == 1 ? '' : 'e'} lub anulowany${count == 1 ? '' : 'e'}.'
      : 'Cannot forward: $count attachment${count == 1 ? ' is' : 's are'} unavailable or cancelled.';
  String get identity => _pl ? 'Tożsamość' : 'Identity';
  String get preparingPrivateSpace => _pl
      ? 'Przygotowywanie prywatnej przestrzeni'
      : 'Preparing your private space';
  String get preparingPrivateSpaceDescription => _pl
      ? 'Konfigurowanie szyfrowanego magazynu i bezpiecznej komunikacji. Możesz pozostawić ten ekran otwarty.'
      : 'Setting up encrypted storage and secure communication. You can safely leave this screen open.';
  String bootstrapProgress(int ready, int total, String elapsed) => _pl
      ? '$ready z $total kontroli bezpieczeństwa ukończonych  •  $elapsed'
      : '$ready of $total secure checks complete  •  $elapsed';
  String bootstrapAttempt(String label, int attempt) =>
      _pl ? '$label · próba $attempt' : '$label · attempt $attempt';
  String get restartApplication =>
      _pl ? 'Uruchom aplikację ponownie' : 'Restart application';
  String bootstrapStepLabel(String id) => switch (id) {
    'local_storage' => _pl ? 'Magazyn lokalny' : 'Local storage',
    'device_identity' => _pl ? 'Tożsamość urządzenia' : 'Device identity',
    'communication_runtime' =>
      _pl ? 'Runtime komunikacji' : 'Communication runtime',
    'incoming_reachability' =>
      _pl ? 'Dostępność połączeń przychodzących' : 'Incoming reachability',
    'rendezvous' => _pl ? 'Rendezvous parowania' : 'Pairing rendezvous',
    _ => id,
  };
  String bootstrapStateDescription(
    String id,
    BootstrapStepState value,
    String? code,
  ) {
    if (value == BootstrapStepState.running ||
        value == BootstrapStepState.verifying) {
      if (id == 'communication_runtime') {
        return switch (code) {
          'COMMUNICATION_RETRYING' =>
            _pl
                ? 'Provider komunikacji ponownie się łączy…'
                : 'The communication provider is reconnecting…',
          'COMMUNICATION_FAILED' =>
            _pl
                ? 'Provider komunikacji wymaga uwagi…'
                : 'The communication provider needs attention…',
          _ =>
            _pl
                ? 'Przygotowywanie wybranego providera komunikacji…'
                : 'Preparing the selected communication provider…',
        };
      }
      if (id == 'incoming_reachability') {
        return code == 'INCOMING_REACHABILITY_PENDING'
            ? (_pl
                  ? 'Przygotowywanie urządzenia do komunikacji przychodzącej…'
                  : 'Preparing this device for incoming communication…')
            : (_pl
                  ? 'Przygotowywanie dostępności połączeń przychodzących…'
                  : 'Preparing incoming reachability…');
      }
      return switch (id) {
        'local_storage' =>
          _pl
              ? 'Otwieranie szyfrowanego magazynu i sprawdzanie schematu…'
              : 'Opening encrypted storage and checking its schema…',
        'device_identity' =>
          _pl
              ? 'Wczytywanie kluczy urządzenia i obliczanie odcisku…'
              : 'Loading device keys and calculating fingerprint…',
        'incoming_reachability' =>
          _pl
              ? 'Przygotowywanie trasy dla komunikacji przychodzącej…'
              : 'Preparing a route for incoming communication…',
        'rendezvous' =>
          _pl
              ? 'Sprawdzanie rendezvous parowania…'
              : 'Testing the pairing rendezvous…',
        _ => _pl ? 'Bezpieczne wykonywanie pracy…' : 'Working securely…',
      };
    }
    return switch (value) {
      BootstrapStepState.ready => switch (id) {
        'local_storage' =>
          _pl
              ? 'Szyfrowana baza danych jest otwarta'
              : 'Encrypted database is open',
        'device_identity' =>
          _pl
              ? 'Tożsamość urządzenia jest chroniona i gotowa'
              : 'Device identity is protected and ready',
        'communication_runtime' =>
          _pl
              ? 'Runtime komunikacji jest gotowy'
              : 'Communication runtime is ready',
        'incoming_reachability' =>
          _pl
              ? 'Urządzenie może odbierać komunikację'
              : 'This device can receive communication',
        'rendezvous' =>
          _pl
              ? 'Rendezvous parowania jest dostępne'
              : 'Pairing rendezvous is reachable',
        _ => _pl ? 'Chronione i gotowe' : 'Protected and ready',
      },
      BootstrapStepState.degraded =>
        _pl
            ? 'Tymczasowo niedostępne; ponawianie'
            : 'Temporarily unavailable; retrying',
      BootstrapStepState.failed when code == 'COMMUNICATION_RESTART_REQUIRED' =>
        _pl
            ? 'Provider komunikacji nie zatrzymał się poprawnie; uruchom aplikację ponownie przed kolejną próbą'
            : 'The communication provider did not stop safely; restart the application before retrying',
      BootstrapStepState.blocked =>
        _pl
            ? 'Oczekiwanie na gotowość komunikacji'
            : 'Waiting for communication to become ready',
      BootstrapStepState.failed =>
        _pl
            ? 'Wymaga uwagi: ${code ?? 'COMMUNICATION_RUNTIME_FAILED'}'
            : 'Needs attention: ${code ?? 'COMMUNICATION_RUNTIME_FAILED'}',
      _ =>
        _pl
            ? 'Oczekiwanie na poprzednią kontrolę bezpieczeństwa'
            : 'Waiting for the previous secure check',
    };
  }

  String get closeInvitationDescription => _pl
      ? 'Możesz zamknąć to okno i korzystać z aplikacji. Zaproszenie pojawi się tutaj automatycznie, gdy połączenie będzie gotowe.'
      : 'Close this window to continue using the application. The invitation will appear here automatically when the connection is ready.';
  String get remoteIdentityTitle =>
      _pl ? 'Tożsamość zdalna' : 'Remote identity';
  String get contactConnected =>
      _pl ? 'Kontakt połączony' : 'Contact connected';
  String get contactConnectedDescription => _pl
      ? 'Zaproszenie zostało zaakceptowane, a kontakt jest gotowy do rozmowy.'
      : 'The invitation was accepted and this contact is ready to chat.';
  String get verifyFingerprintBeforeAccepting => _pl
      ? 'Urządzenie dołączyło do tego zaproszenia. Sprawdź odcisk przed zaakceptowaniem kontaktu.'
      : 'A device joined this invitation. Verify the fingerprint before accepting the contact.';
  String get joinRequestWaiting => _pl
      ? 'Twoje żądanie czeka na weryfikację i akceptację przez właściciela zaproszenia.'
      : 'Your request is waiting for the invitation owner to verify and accept it.';
  String get collapseNavigation =>
      _pl ? 'Zwiń nawigację' : 'Collapse navigation';
  String get expandNavigation => _pl ? 'Rozwiń nawigację' : 'Expand navigation';
  String buildServiceSummary(String build, String service) =>
      'build $build  •  $service';
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

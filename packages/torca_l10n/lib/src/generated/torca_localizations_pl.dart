// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'torca_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Polish (`pl`).
class TorcaLocalizationsPl extends TorcaLocalizations {
  TorcaLocalizationsPl([String locale = 'pl']) : super(locale);

  @override
  String get aboutTorca => 'O Torca';

  @override
  String get accept => 'Akceptuj';

  @override
  String get activeDemands => 'Aktywne zapotrzebowania';

  @override
  String get activeInvitationsDescription =>
      'Aktywne zaproszenia i prosby o parowanie pojawia sie tutaj.';

  @override
  String get activeLeases => 'Aktywne dzierżawy';

  @override
  String get activeTransfers => 'Aktywne';

  @override
  String get allOperations => 'Wszystko';

  @override
  String get allowAll => 'Zezwalaj na wszystko';

  @override
  String get allowDelayedBackgroundDelivery =>
      'Zezwalaj na opóźnione dostarczanie w tle';

  @override
  String get allowDelayedBackgroundDeliveryDescription =>
      'Wymagane, aby tryb automatyczny lub oszczędzania baterii mógł wstrzymać runtime komunikacji, gdy aplikacja jest bezczynna.';

  @override
  String get alwaysAvailable => 'Zawsze dostępny';

  @override
  String get appearance => 'Wygląd';

  @override
  String get appearanceTitle => 'Wygląd';

  @override
  String get applicationMenu => 'Menu aplikacji';

  @override
  String get archiveConversation => 'Archiwizuj rozmowę';

  @override
  String get attachFiles => 'Dołącz pliki';

  @override
  String get attachmentAckTimeout => 'oczekiwanie na potwierdzenie kontaktu';

  @override
  String get attachmentDependencyMissing => 'oczekiwanie na rozmowę';

  @override
  String get attachmentIntegrityFailed => 'błąd integralności';

  @override
  String get attachmentMessagePending => 'oczekiwanie na wiadomość';

  @override
  String get attachmentOperationFailed => 'Operacja zalacznika nieudana';

  @override
  String get attachmentPeerUnavailable => 'kontakt niedostępny';

  @override
  String get attachmentRetryAvailable => 'dostępna ponowna próba';

  @override
  String get attachmentSaved => 'Zalacznik zapisany';

  @override
  String get attachmentStorageFailed => 'błąd lokalnego zapisu';

  @override
  String get attachmentSyncing => 'Synchronizacja zalacznika…';

  @override
  String attachmentsQueued(num count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dodano do kolejki: $count zalacznik�w',
      one: 'Dodano do kolejki: 1 zalacznik',
    );
    return '$_temp0';
  }

  @override
  String get audio => 'Dźwięk';

  @override
  String get audioDeviceUnavailable =>
      'Nie można użyć wybranego urządzenia audio.';

  @override
  String get audioOutput => 'Wyjście audio';

  @override
  String get automatic => 'Automatycznie';

  @override
  String get availabilityMode => 'Tryb dostępności';

  @override
  String get batteryAvailability => 'Bateria i dostępność';

  @override
  String get batteryObservation => 'Obserwacja baterii';

  @override
  String get batterySaver => 'Oszczędzanie baterii';

  @override
  String get batterySettingsDescription =>
      'Wybierz, kiedy Torca może odroczyć pracę w tle. Przychodzące zadania nigdy nie są po cichu odrzucane.';

  @override
  String get batteryTab => 'Bateria';

  @override
  String get blockContact => 'Zablokuj kontakt';

  @override
  String get blockContactDescription =>
      'Torca zamknie połączenie z tym kontaktem i nie połączy się ponownie, dopóki go nie odblokujesz.';

  @override
  String blockContactTitle(Object name) {
    return 'Zablokować kontakt $name?';
  }

  @override
  String get blocked => 'Zablokowany';

  @override
  String get blockedSendBlocked =>
      'Kontakt jest zablokowany. Odblokuj go, aby wysłać wiadomość.';

  @override
  String get bookmarkMessage => 'Zapisz wiadomość';

  @override
  String bootstrapAttempt(String label, int attempt) {
    return '$label · próba $attempt';
  }

  @override
  String bootstrapProgress(int ready, int total, String elapsed) {
    return '$ready z $total kontroli bezpieczeństwa ukończonych  •  $elapsed';
  }

  @override
  String bootstrapStateDescription(Object code, Object id, Object value) {
    return '$id $value $code';
  }

  @override
  String bootstrapStepLabel(Object id) {
    return '$id';
  }

  @override
  String get build => 'Build';

  @override
  String get buildAndConnectionInfo => 'Informacje o buildzie i polaczeniu';

  @override
  String buildLabel(Object build) {
    return 'build $build';
  }

  @override
  String buildServiceSummary(Object build, Object service) {
    return '$build $service';
  }

  @override
  String buildTooltip(Object build, Object providerService) {
    return 'Build $build\nWersja usługi providera: $providerService';
  }

  @override
  String get cancel => 'Anuluj';

  @override
  String get cancelInvitation => 'Anuluj zaproszenie';

  @override
  String get cancelMessage => 'Anuluj wiadomosc';

  @override
  String get cancelRequest => 'Anuluj zadanie';

  @override
  String get cancelled => 'Anulowano';

  @override
  String get chats => 'Rozmowy';

  @override
  String get checkingInvitation => 'Sprawdzanie zaproszenia...';

  @override
  String get chooseConversation => 'Wybierz rozmowę';

  @override
  String get chooseLanguage => 'Wybierz język';

  @override
  String get chooseLanguagePolish => 'Choose Language Polish';

  @override
  String get chooseNickname => 'Wybierz pseudonim';

  @override
  String get clearConversationHistory => 'Wyczyść historię rozmowy';

  @override
  String get clearSearch => 'Wyczysc wyszukiwanie';

  @override
  String get close => 'Zamknij';

  @override
  String get closeInvitationDescription =>
      'Możesz zamknąć to okno i korzystać z aplikacji. Zaproszenie pojawi się tutaj automatycznie, gdy połączenie będzie gotowe.';

  @override
  String get closeScanner => 'Zamknij skaner';

  @override
  String get closeSearch => 'Zamknij wyszukiwanie';

  @override
  String get closeToTray => 'Zamykaj do zasobnika';

  @override
  String get closeToTrayDescription =>
      'Pozostaw Torca uruchomioną po zamknięciu głównego okna. Wyłącz, aby zamknięcie okna kończyło aplikację.';

  @override
  String get closeTooltip => 'Zamknij';

  @override
  String get collapseNavigation => 'Zwiń nawigację';

  @override
  String get comfortableDensity => 'Gęstość wygodna';

  @override
  String get communicationProvider => 'Provider komunikacji';

  @override
  String get communicationState => 'Stan komunikacji';

  @override
  String get compactDensity => 'Gęstość kompaktowa';

  @override
  String get completedTransfers => 'Zakończone';

  @override
  String get connecting => 'Laczenie';

  @override
  String connectingPeerThrough(String provider) {
    return '$provider';
  }

  @override
  String get connection => 'Połączenie';

  @override
  String get connectionDetails => 'Szczegóły połączenia';

  @override
  String get connectionDetailsTitle => 'Szczegoly polaczenia';

  @override
  String connectionEvidenceNote(String provider) {
    return '$provider';
  }

  @override
  String connectionQuality(Object quality, Object rtt) {
    return 'Jakość połączenia $quality$rtt';
  }

  @override
  String get connectionSelfTest => 'Test polaczenia';

  @override
  String get consecutiveFailures => 'Kolejne bledy';

  @override
  String contactAcceptedJoin(Object name) {
    return '$name zaakceptował(a) zaproszenie';
  }

  @override
  String get contactActions => 'Akcje kontaktu';

  @override
  String contactAddedToContacts(Object name) {
    return '$name dodano do kontaktów';
  }

  @override
  String get contactBlocked => 'Kontakt jest zablokowany';

  @override
  String get contactConnected => 'Kontakt połączony';

  @override
  String get contactConnectedDescription =>
      'Zaproszenie zostało zaakceptowane, a kontakt jest gotowy do rozmowy.';

  @override
  String get contactDetails => 'Szczegoly kontaktu';

  @override
  String get contactInformation => 'Informacje o kontakcie';

  @override
  String get contactLabel => 'Kontakt';

  @override
  String get contactUnavailable => 'Ten kontakt nie jest juz dostepny.';

  @override
  String get contacts => 'Kontakty';

  @override
  String contactsCount(num count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count kontakt�w',
      one: '1 kontakt',
    );
    return '$_temp0';
  }

  @override
  String get continueLabel => 'Dalej';

  @override
  String get contract => 'Kontrakt';

  @override
  String get contractDecodeFailed =>
      'Klient i runtime używają niezgodnych danych. Zbuduj i wdroż oba ponownie.';

  @override
  String get contractSnapshotReadable => 'Snapshot kontraktu czytelny';

  @override
  String get conversationActions => 'Akcje rozmowy';

  @override
  String get copy => 'Kopiuj';

  @override
  String get copyCode => 'Kopiuj zaproszenie';

  @override
  String get copyFingerprint => 'Kopiuj odcisk';

  @override
  String get couldNotBlockContact => 'Nie udało się zablokować kontaktu';

  @override
  String get couldNotForwardMessage => 'Nie udało się przekazać wiadomości';

  @override
  String get couldNotQueueAttachment =>
      'Nie udalo sie dodac zalacznika do kolejki';

  @override
  String get couldNotRemoveContact => 'Nie udało się usunąć kontaktu';

  @override
  String get couldNotRenameContact => 'Nie udało się zmienić nazwy kontaktu';

  @override
  String get couldNotSaveNickname => 'Nie udało się zapisać pseudonimu';

  @override
  String couldNotStartConversation(Object name) {
    return 'Nie udało się rozpocząć rozmowy z $name.';
  }

  @override
  String get couldNotStartRadio => 'Nie udalo sie rozpoczac transmisji';

  @override
  String get couldNotUnblockContact => 'Nie udało się odblokować kontaktu';

  @override
  String get couldNotUpdateRadio => 'Nie udalo sie zmienic trybu radio';

  @override
  String get couldNotUpdateReaction => 'Nie udało się wysłać reakcji';

  @override
  String get country => 'Skąd jesteś?';

  @override
  String get createInvitationForContact =>
      'Utworz zaproszenie, aby dodac prywatny kontakt.';

  @override
  String get createManageInvitations =>
      'Tworz i zarzadzaj krotkimi, prywatnymi zaproszeniami.';

  @override
  String get createdInvitation => 'Utworzone zaproszenie';

  @override
  String get dark => 'Ciemny';

  @override
  String defaultAudioDevice(Object name) {
    return '$name (domyślne)';
  }

  @override
  String get deleteMessage => 'Usun dla obu stron';

  @override
  String get deleteMessageTitle => 'Usunac wiadomosc?';

  @override
  String get delivered => 'Dostarczono';

  @override
  String get deliveryFailed => 'Dostarczenie nieudane';

  @override
  String get desktop => 'Pulpit';

  @override
  String deviceFingerprint(Object fingerprint) {
    return 'Odcisk urządzenia\n$fingerprint';
  }

  @override
  String get diagnostics => 'Diagnostyka';

  @override
  String get diagnosticsExported => 'Diagnostyka wyeksportowana';

  @override
  String get diagnosticsStream => 'Strumien diagnostyczny';

  @override
  String directPeerLinksReady(Object ready, Object total) {
    return '$ready z $total bezposrednich polaczen gotowych';
  }

  @override
  String get directPeers => 'Bezposrednie wezly';

  @override
  String directProviderContact(String provider) {
    return '$provider';
  }

  @override
  String get displayName => 'Nazwa wyświetlana';

  @override
  String get documentTransfers => 'Dokumenty';

  @override
  String get done => 'Gotowe';

  @override
  String get draft => 'Szkic';

  @override
  String get editMessage => 'Edytuj wiadomosc';

  @override
  String get emoji => 'Emoji';

  @override
  String get enableNotifications => 'Włącz powiadomienia';

  @override
  String get encrypting => 'Szyfrowanie';

  @override
  String get endpoint => 'Endpoint';

  @override
  String get englishCountry => 'Anglia';

  @override
  String get enterSixCharacterCode =>
      'Wpisz szescioznakowy kod lub zeskanuj kod QR.';

  @override
  String get excellent => 'Doskonaly';

  @override
  String get expandNavigation => 'Rozwiń nawigację';

  @override
  String get exportDiagnostics => 'Eksportuj diagnostyke';

  @override
  String get exportFailed => 'Eksport nieudany';

  @override
  String get exportTorcaDiagnostics => 'Eksportuj diagnostykę Torca';

  @override
  String get fair => 'Sredni';

  @override
  String get fileTransfers => 'Pliki';

  @override
  String get finalizingContact => 'Finalizowanie bezpiecznego kontaktu…';

  @override
  String get fingerprint => 'Odcisk';

  @override
  String get fingerprintCopied => 'Odcisk skopiowany';

  @override
  String get focusedOnly => 'Animuj tylko aktywne widoki';

  @override
  String get followSystem => 'Zgodnie z ustawieniami systemu';

  @override
  String get forwardMessage => 'Przekaż wiadomość';

  @override
  String forwardNoAvailableAttachments(Object count) {
    return '$count';
  }

  @override
  String forwardSkippedAttachments(Object count) {
    return '$count';
  }

  @override
  String get fullAnimation => 'Pełna animacja';

  @override
  String get generateInvitation => 'Wygeneruj zaproszenie';

  @override
  String get generatingInvitation => 'Generowanie…';

  @override
  String get good => 'Dobry';

  @override
  String get holdToRecordVoiceClip => 'Przytrzymaj, aby nagrac klip glosowy';

  @override
  String get identicalDeadlineReplacements => 'Identyczne zastąpienia terminów';

  @override
  String get identity => 'Tożsamość';

  @override
  String get identityChanged =>
      'Tożsamość kontaktu uległa zmianie. Sprawdź numer bezpieczeństwa.';

  @override
  String get identityChangedSendBlocked =>
      'Wysyłanie jest wstrzymane do czasu ponownej weryfikacji kontaktu.';

  @override
  String get incidentDescription =>
      'Uruchom autotest, oznacz bieżący stan i wyeksportuj zanonimizowany zrzut diagnostyczny. Treści wiadomości, załączniki, audio i sekrety nie są dołączane.';

  @override
  String get incidentSnapshotSaved =>
      'Zrzut incydentu zapisano w lokalnej diagnostyce tego uruchomienia.';

  @override
  String get incidentTab => 'Incydent';

  @override
  String get incidentTools => 'Narzędzia incydentu';

  @override
  String get incomingMessage => 'Wiadomość przychodząca';

  @override
  String get incompatibleStorageEpoch =>
      'Zaszyfrowany profil lokalny jest niezgodny. Jawnie zresetuj lokalne dane Torca przed kontynuowaniem.';

  @override
  String get instantMode => 'Tryb natychmiastowego polaczenia';

  @override
  String get instantModeEnabled => 'Tryb natychmiastowego polaczenia wlaczony';

  @override
  String get invalidInput => 'Podana wartość jest nieprawidłowa.';

  @override
  String get invitationCode => 'Kod zaproszenia';

  @override
  String get invitationCodeCopied => 'Pełne zaproszenie skopiowane';

  @override
  String invitationCodeLabel(Object code) {
    return 'Kod $code';
  }

  @override
  String invitationExpiresIn(Object countdown) {
    return 'Wygasa za $countdown';
  }

  @override
  String get invitationGenerating => 'Generowanie prywatnego zaproszenia...';

  @override
  String get invitationJoinSent =>
      'Zadanie dolaczenia wyslane. Otrzymasz powiadomienie po akceptacji.';

  @override
  String get invitationOperationFailed =>
      'Operacja zaproszenia nie powiodla sie';

  @override
  String get invitationQueued =>
      'Zaproszenie dodane do kolejki bezpiecznej sieci.';

  @override
  String get invitationSavedLocally =>
      'Zapisano lokalnie. Ponowimy, gdy wybrany dostawca komunikacji bedzie gotowy.';

  @override
  String get invitationWaitingForNetwork => 'Zaproszenie oczekuje na siec.';

  @override
  String get invitations => 'Zaproszenia';

  @override
  String get itemAlreadyExists => 'Ten element już istnieje.';

  @override
  String get itemNotFound => 'Element nie jest już dostępny.';

  @override
  String get joinInvitation => 'Dolacz do zaproszenia';

  @override
  String get joinRequestWaiting =>
      'Twoje żądanie czeka na weryfikację i akceptację przez właściciela zaproszenia.';

  @override
  String get joinedInvitation => 'Dolaczone zaproszenie';

  @override
  String get jumpToLatest => 'Przejdź do najnowszej wiadomości';

  @override
  String get language => 'Język';

  @override
  String get languageEnglish => 'Angielski';

  @override
  String get languagePolish => 'Polski';

  @override
  String get languageSystem => 'Język systemowy';

  @override
  String get languageTitle => 'Język';

  @override
  String get lastSeen => 'Ostatnio widziany';

  @override
  String lastSeenAt(Object time) {
    return 'Ostatnio $time';
  }

  @override
  String get lastSuccessfulProbe => 'Ostatnia udana sonda';

  @override
  String get leaseReasons => 'Powody dzierżaw';

  @override
  String get light => 'Jasny';

  @override
  String get loadCurrentRunLogs => 'Wczytaj logi bieżącego uruchomienia';

  @override
  String get loaded => 'Zaladowano';

  @override
  String get localIdentity => 'Tożsamość lokalna';

  @override
  String get localIdentityCheck => 'Lokalna tozsamosc';

  @override
  String get localIdentityNotReady =>
      'Lokalna tożsamość nie jest jeszcze gotowa';

  @override
  String get localName => 'Nazwa lokalna';

  @override
  String get logsTab => 'Logi';

  @override
  String get markConversationRead => 'Oznacz jako przeczytane';

  @override
  String get markIncident => 'Oznacz incydent';

  @override
  String get mediaTransfers => 'Media';

  @override
  String get message => 'Wiadomość';

  @override
  String get messageActions => 'Akcje wiadomości';

  @override
  String get messageCancelled => 'Wiadomosc anulowana';

  @override
  String get messageCopied => 'Wiadomosc skopiowana';

  @override
  String get messageDeleted => 'Wiadomosc usunieta';

  @override
  String get messageDetails => 'Szczegoly wiadomosci';

  @override
  String get messageEdited => 'Wiadomosc zmieniona';

  @override
  String get messageForwarded => 'Wiadomość przekazana';

  @override
  String get messageQueued =>
      'W kolejce — oczekiwanie na bezpośrednie połączenie';

  @override
  String get messageSenderContact => 'Kontakt';

  @override
  String get messageSenderYou => 'Ty';

  @override
  String messageTooLong(int maximum) {
    return 'Wiadomość może mieć maksymalnie $maximum znaków.';
  }

  @override
  String get meteredTransfers => 'Transfery w sieci taryfowej';

  @override
  String get microphone => 'Mikrofon';

  @override
  String get microphonePermissionRequired =>
      'Dostep do mikrofonu jest wymagany do nadawania.';

  @override
  String get modern => 'Nowoczesny';

  @override
  String get muteConversation => 'Wycisz rozmowę';

  @override
  String get nativeBridge => 'Most natywny';

  @override
  String get nativeLogTails => 'Logi natywne';

  @override
  String get nativeLogTailsDescription =>
      'Wczytuje ograniczony, zanonimizowany fragment bieżących logów natywnych. Odczyt nie uruchamia ciągłego monitorowania.';

  @override
  String get networkUnavailable =>
      'Wybrane połączenie komunikacyjne jest obecnie niedostępne.';

  @override
  String get never => 'Nigdy';

  @override
  String get newContact => 'Nowy kontakt';

  @override
  String get newDevice => 'Nowe urządzenie';

  @override
  String get newMessages => 'Nowe wiadomości';

  @override
  String get newPairing => 'Nowe parowanie';

  @override
  String get newPairingRequest => 'Nowa prośba parowania';

  @override
  String get newPrivateMessage => 'Nowa prywatna wiadomość';

  @override
  String get nextDeadline => 'Następny termin';

  @override
  String get nickname => 'Pseudonim';

  @override
  String get nicknameIntro =>
      'Bezpieczny provider komunikacji jest gotowy. Ta nazwa będzie widoczna dla kontaktów.';

  @override
  String get nicknameRequired => 'Pseudonim jest wymagany';

  @override
  String get noActiveTransfers => 'Brak aktywnych transferow.';

  @override
  String get noChatsMatch => 'Brak rozmow pasujacych do wyszukiwania';

  @override
  String get noContactsPaired => 'Brak sparowanych kontaktow';

  @override
  String get noContactsYet => 'Brak kontaktow';

  @override
  String get noForwardableContent =>
      'Ta wiadomość nie zawiera treści możliwej do przekazania.';

  @override
  String get noInvitations => 'Brak zaproszen';

  @override
  String get noMatchingMessages => 'Brak pasujących wiadomości.';

  @override
  String get noMessagesYet => 'Brak wiadomosci';

  @override
  String get noMessagesYetDescription =>
      'Wiadomosci sa wysylane bezposrednio przez wybrany provider komunikacji.';

  @override
  String get noReadableHealthEvents => 'Brak czytelnych zdarzen zdrowia';

  @override
  String get notInitialized => 'Nie zainicjalizowano';

  @override
  String get notMeasured => 'Nie zmierzono';

  @override
  String get notificationPrivacy =>
      'Pokazuj powiadomienia o prywatnych wiadomościach bez ich treści.';

  @override
  String get notifications => 'Powiadomienia';

  @override
  String get notificationsTitle => 'Powiadomienia';

  @override
  String get observationRecording => 'rejestrowanie';

  @override
  String get observationRecordingDescription =>
      'Rejestrowanie zmian od punktu bazowego obserwacji.';

  @override
  String get observationState => 'Stan';

  @override
  String get observationStopped => 'zatrzymano';

  @override
  String get observationStoppedDescription =>
      'Uruchom przed scenariuszem bezczynności lub odzyskiwania, aby zapisać nową pracę.';

  @override
  String get observationWork => 'Praca';

  @override
  String get offlineShort => 'Offline';

  @override
  String get online => 'Online';

  @override
  String get open => 'Otworz';

  @override
  String get openChat => 'Otworz czat';

  @override
  String get openConversation => 'Otworz rozmowe';

  @override
  String get operationFailed => 'Nie udało się wykonać operacji.';

  @override
  String get originalMessageUnavailable =>
      'Oryginalna wiadomość jest niedostępna';

  @override
  String get outgoingMessage => 'Wiadomość wychodząca';

  @override
  String get p2pShort => 'P2P';

  @override
  String get pairContact => 'Połącz kontakt';

  @override
  String get pairContactHint => 'Polacz kontakt, aby rozpoczac rozmowe.';

  @override
  String get pairingBootstrapRequired =>
      'Dla tego providera zeskanuj kod QR albo wklej pelne zaproszenie.';

  @override
  String get pairingCompletedMessage =>
      'Kontakt został bezpiecznie dodany. Otworzyć teraz prywatną rozmowę?';

  @override
  String get pairingExpired => 'Zaproszenie wygasło.';

  @override
  String get pairingInactiveMessage =>
      'To zaproszenie nie jest już aktywne. Drugie urządzenie otrzyma ten sam stan końcowy.';

  @override
  String get pairingProviderMismatch =>
      'To zaproszenie pochodzi od innego providera komunikacji.';

  @override
  String get pairingQrSemanticLabel => 'Kod QR zaproszenia do parowania Torca';

  @override
  String get pairingRequestDescription =>
      'To urządzenie dołączyło do Twojego zaproszenia. Sprawdź dane kontaktu przed akceptacją.';

  @override
  String pairingStateLabel(String state) {
    String _temp0 = intl.Intl.selectLogic(state, {
      'open': 'Otwarte',
      'peer_joined': 'Kontakt dolaczyl',
      'awaiting_approval': 'Czeka na akceptacje',
      'approved': 'Zaakceptowane',
      'completed': 'Polaczone',
      'rejected': 'Odrzucone',
      'cancelled': 'Anulowane',
      'expired': 'Wygasle',
      'unknown': 'Nieznany stan',
      'other': 'Nieznany stan',
    });
    return '$_temp0';
  }

  @override
  String get pauseAll => 'Wstrzymuj wszystkie transfery';

  @override
  String get pauseLarge => 'Wstrzymuj duże pliki';

  @override
  String get peerOffline => 'Kontakt offline';

  @override
  String get peerState => 'Stan P2P';

  @override
  String get pendingOperations => 'Oczekujące';

  @override
  String get pinConversation => 'Przypnij rozmowę';

  @override
  String get playVoiceMessage => 'Odtworz wiadomosc glosowa';

  @override
  String get polishCountry => 'Polska';

  @override
  String get poor => 'Slaby';

  @override
  String get preparingDownload => 'Przygotowanie pobierania';

  @override
  String get preparingPrivateSpace => 'Przygotowywanie prywatnej przestrzeni';

  @override
  String get preparingPrivateSpaceDescription =>
      'Konfigurowanie szyfrowanego magazynu i bezpiecznej komunikacji. Możesz pozostawić ten ekran otwarty.';

  @override
  String get preparingSecureCopy => 'Przygotowanie bezpiecznej kopii';

  @override
  String get preparingUpload => 'Przygotowanie wysylania';

  @override
  String get presence => 'Obecnosc';

  @override
  String get privacy => 'Prywatność';

  @override
  String get privacyTitle => 'Prywatność';

  @override
  String get productVersion => 'Wersja produktu';

  @override
  String get profileNotReady => 'Bezpieczny profil nie jest jeszcze gotowy.';

  @override
  String get providerEndpoint => 'Endpoint providera';

  @override
  String get providerEndpointAvailable => 'Dostępny';

  @override
  String get providerEndpointUnavailable => 'Niedostępny';

  @override
  String providerName(String provider) {
    return '$provider';
  }

  @override
  String providerReady(String provider) {
    return '$provider';
  }

  @override
  String providerReconnecting(String provider) {
    return '$provider';
  }

  @override
  String providerStarting(String provider) {
    return '$provider';
  }

  @override
  String providerStateLabel(String provider, String state) {
    return '$provider $state';
  }

  @override
  String get published => 'Opublikowano';

  @override
  String get quality => 'Jakość';

  @override
  String get queued => 'W kolejce';

  @override
  String get radioChannelInterrupted => 'Kanal radio zostal przerwany';

  @override
  String get radioChannelReady => 'Prywatny kanal radio jest gotowy';

  @override
  String get radioChannelRestored => 'Kanal radio zostal przywrocony';

  @override
  String get radioConnecting => 'Laczenie prywatnego kanalu audio...';

  @override
  String radioDisabledBy(Object actor) {
    return '$actor wylaczyl(a) tryb radio';
  }

  @override
  String radioEnabledBy(Object actor) {
    return '$actor wlaczyl(a) tryb radio';
  }

  @override
  String get radioMode => 'Tryb radio';

  @override
  String get radioModeDescription =>
      'Krotkie, maksymalnie 10-sekundowe transmisje PTT. Radio dziala dopiero, gdy obie strony je wlacza.';

  @override
  String get radioReady => 'Przytrzymaj, aby mowic';

  @override
  String radioReceiving(Object name) {
    return '$name nadaje';
  }

  @override
  String get radioReconnecting => 'Radio laczy sie ponownie...';

  @override
  String get radioRequestingFloor => 'Rezerwowanie kanalu...';

  @override
  String get radioTransmitting => 'Nadajesz';

  @override
  String radioTransportFailure(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'endpoint_unavailable': 'brak punktu koncowego',
      'connect_timeout': 'przekroczono czas laczenia',
      'stream_reset': 'strumien zostal przerwany',
      'idle_timeout': 'kanal wygasl podczas bezczynnosci',
      'network_changed': 'zmienila sie siec',
      'worker_unavailable': 'worker audio jest niedostepny',
      'protocol': 'blad protokolu',
      'other': 'nieznany blad transportu',
    });
    return 'Radio: $_temp0';
  }

  @override
  String get radioUnavailable => 'Radio jest chwilowo niedostepne';

  @override
  String get radioWaitingForPeer => 'Oczekiwanie, az kontakt wlaczy radio';

  @override
  String get rawDiagnostics => 'Surowa diagnostyka';

  @override
  String get reactToMessage => 'Reakcja';

  @override
  String get read => 'Odczytano';

  @override
  String get receivingSecurely => 'Bezpieczny odbior';

  @override
  String get recentEmoji => 'Ostatnio używane';

  @override
  String get recentInvitations => 'Ostatnie zaproszenia';

  @override
  String get reconnectAttempts => 'Proby ponownego polaczenia';

  @override
  String get reconnecting => 'Ponowne laczenie';

  @override
  String reconnectingPeerThrough(String provider) {
    return '$provider';
  }

  @override
  String get reconnectingShort => 'Laczenie';

  @override
  String get recordingTransfers => 'Nagrania';

  @override
  String get redactedDeveloperEventStream =>
      'Zanonimizowany strumien zdarzen deweloperskich';

  @override
  String get redactedHealthEventsReadable =>
      'Zanonimizowane zdarzenia zdrowia czytelne';

  @override
  String get redactedSchedulerDescription =>
      'Zanonimizowane wyjaśnienie harmonogramu; identyfikatory kontaktów nie są tutaj wyświetlane.';

  @override
  String get reduceMotion => 'Ogranicz ruch';

  @override
  String get refresh => 'Odswiez';

  @override
  String get refreshProviderRoute => 'Odśwież trasę providera';

  @override
  String get regressionScore => 'Wynik regresji';

  @override
  String get reject => 'Odrzuc';

  @override
  String remoteIdentity(String id) {
    return 'Tozsamosc $id';
  }

  @override
  String get remoteIdentityTitle => 'Tożsamość zdalna';

  @override
  String get remove => 'Usuń';

  @override
  String get removeAttachment => 'Usun zalacznik';

  @override
  String get removeBookmark => 'Usuń zakładkę';

  @override
  String get removeContact => 'Usuń kontakt';

  @override
  String get removeContactDescription =>
      'Usuwa to lokalną relację, historię rozmowy, oczekujące operacje i chronione dane uwierzytelniające kontaktu.';

  @override
  String removeContactTitle(Object name) {
    return 'Usunąć kontakt $name?';
  }

  @override
  String get renameContact => 'Zmień nazwę kontaktu';

  @override
  String get reply => 'Odpowiedź';

  @override
  String get resetBaseline => 'Zresetuj punkt bazowy';

  @override
  String get resetVerification => 'Resetuj weryfikację';

  @override
  String get restartApplication => 'Uruchom aplikację ponownie';

  @override
  String get restoreConversation => 'Przywróć rozmowę';

  @override
  String get retry => 'Ponów';

  @override
  String get retryGeneration => 'Ponow generowanie';

  @override
  String get retryNow => 'Spróbuj ponownie';

  @override
  String get retrying => 'Ponawianie…';

  @override
  String get roundTrip => 'Opoznienie';

  @override
  String get route => 'Trasa providera';

  @override
  String get routeRefreshRequested => 'Zażądano odświeżenia trasy providera.';

  @override
  String get routeRefreshRequired =>
      'Trasa połączenia jest odświeżana. Spróbuj ponownie za chwilę.';

  @override
  String get runSelfTest => 'Uruchom test';

  @override
  String get runtimeHealth => 'Stan runtime';

  @override
  String runtimeNotReadyDiagnostic(Object provider) {
    return '$provider';
  }

  @override
  String get runtimePreparationFailed =>
      'Nie udalo sie przygotowac lokalnego szyfrowanego runtime. Tozsamosc nie zostala zmieniona.';

  @override
  String get runtimeTab => 'Runtime';

  @override
  String get runtimeUnavailable =>
      'Bezpieczny runtime Torca jest obecnie niedostępny.';

  @override
  String get sampleContactName => 'Alice';

  @override
  String get sampleOnline => 'online';

  @override
  String get sampleTime => '14:22';

  @override
  String get save => 'Zapisz';

  @override
  String get saveAs => 'Zapisz jako';

  @override
  String get saveAttachment => 'Zapisz zalacznik';

  @override
  String get saving => 'Zapisywanie…';

  @override
  String get scanQr => 'Skanuj QR';

  @override
  String get scheduledWork => 'Zaplanowana praca';

  @override
  String get searchChats => 'Szukaj rozmow';

  @override
  String get searchConversationHint => 'Szukaj w tej rozmowie';

  @override
  String get searchMessages => 'Szukaj wiadomosci';

  @override
  String searchResultsCount(int count) {
    return 'Wyniki: $count';
  }

  @override
  String get secureRuntimeNotReady => 'Bezpieczne srodowisko nie jest gotowe';

  @override
  String get selectConversation => 'Wybierz rozmowe';

  @override
  String get sendMessage => 'Wyślij wiadomość';

  @override
  String get sendReadReceipts => 'Wysyłaj potwierdzenia odczytu';

  @override
  String get sendReadReceiptsDescription =>
      'Oznaczaj wiadomości lokalnie jako przeczytane, ale pozwól kontaktom zobaczyć stan Read tylko wtedy, gdy ta opcja jest włączona.';

  @override
  String get senderContact => 'Kontakt';

  @override
  String get senderYou => 'Ty';

  @override
  String get sendingSecurely => 'Bezpieczne wysylanie';

  @override
  String get sent => 'Wysłano';

  @override
  String get settings => 'Ustawienia';

  @override
  String get settingsTitle => 'Ustawienia';

  @override
  String get sharedMedia => 'Wspólne pliki i multimedia';

  @override
  String sharedMediaCount(int count) {
    return '$count';
  }

  @override
  String get sourceCommit => 'Commit źródłowy';

  @override
  String get startConversation => 'Rozpocznij rozmowę';

  @override
  String get startObservation => 'Rozpocznij obserwację';

  @override
  String get startingSecureNetwork => 'Uruchamianie komunikacji…';

  @override
  String get startingShort => 'Start';

  @override
  String get state => 'Stan';

  @override
  String get staticIdle => 'Statycznie podczas bezczynności';

  @override
  String get status => 'Stan';

  @override
  String get stopObservation => 'Zatrzymaj obserwację';

  @override
  String get storageEpoch => 'Epoka magazynu';

  @override
  String get storageFailure =>
      'Nie udało się zakończyć operacji na szyfrowanym magazynie.';

  @override
  String get system => 'System';

  @override
  String get systemDefaultAudioDevice => 'Domyślne urządzenie systemowe';

  @override
  String get systemLanguage => 'Język systemowy';

  @override
  String get terminal => 'Terminal';

  @override
  String get today => 'Dzisiaj';

  @override
  String get todayUpper => 'DZISIAJ';

  @override
  String get transferFailed => 'Wysylanie nieudane';

  @override
  String get transfers => 'Transfery';

  @override
  String get transport => 'Transport';

  @override
  String get typeToSearchConversation => 'Wpisz tekst, aby przeszukać rozmowę.';

  @override
  String get unavailable => 'Niedostępne';

  @override
  String get unblockContact => 'Odblokuj kontakt';

  @override
  String get unknown => 'Nieznany';

  @override
  String get unknownCountry => 'Nie podano';

  @override
  String get unmuteConversation => 'Włącz powiadomienia';

  @override
  String get unpinConversation => 'Odepnij rozmowę';

  @override
  String get unverified => 'Niezweryfikowany';

  @override
  String get variant => 'Wariant';

  @override
  String get verification => 'Weryfikacja';

  @override
  String get verified => 'Zweryfikowano';

  @override
  String get verifiedOnDevice => 'Zweryfikowano na urzadzeniu';

  @override
  String get verifyContact => 'Zweryfikuj kontakt';

  @override
  String get verifyFingerprintBeforeAccepting =>
      'Urządzenie dołączyło do tego zaproszenia. Sprawdź odcisk przed zaakceptowaniem kontaktu.';

  @override
  String get visualActivity => 'Aktywność avatara i interfejsu';

  @override
  String voiceClipRecording(Object secondsLeft) {
    return 'Nagrywanie klipu, pozostalo $secondsLeft s';
  }

  @override
  String get voiceClipRecordingFailed =>
      'Nie udalo sie nagrac klipu glosowego.';

  @override
  String get voiceMessage => 'Wiadomosc glosowa';

  @override
  String get voiceMessagePlayed => 'Odtworzono';

  @override
  String get voiceMessageReady => 'Gotowe do odtworzenia';

  @override
  String waitingForDependency(Object dependency) {
    return 'Oczekuje: $dependency';
  }

  @override
  String get waitingForPeer => 'Oczekiwanie na kontakt';

  @override
  String get waitingToReceive => 'Oczekiwanie na odbior';

  @override
  String get wakeSources => 'Źródła wybudzeń';

  @override
  String get whyAwake => 'Dlaczego aktywny';

  @override
  String get yesterday => 'Wczoraj';

  @override
  String get yourIdentity => 'Twoja tożsamość';

  @override
  String get yourInvitation => 'Twoje zaproszenie';

  @override
  String get zeroDelayDeadlines => 'Terminy bez opóźnienia';
}

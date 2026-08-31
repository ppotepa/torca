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
  String get activeDemands => 'Active Demands';

  @override
  String get activeInvitationsDescription =>
      'Aktywne zaproszenia i prosby o parowanie pojawia sie tutaj.';

  @override
  String get activeLeases => 'Active Leases';

  @override
  String get activeTransfers => 'Active Transfers';

  @override
  String get allOperations => 'All Operations';

  @override
  String get allowAll => 'Allow All';

  @override
  String get allowDelayedBackgroundDelivery =>
      'Allow Delayed Background Delivery';

  @override
  String get allowDelayedBackgroundDeliveryDescription =>
      'Allow Delayed Background Delivery Description';

  @override
  String get alwaysAvailable => 'Always Available';

  @override
  String get appearance => 'Wygląd';

  @override
  String get appearanceTitle => 'Wygląd';

  @override
  String get applicationMenu => 'Menu aplikacji';

  @override
  String get archiveConversation => 'Archive Conversation';

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
  String get audio => 'Audio';

  @override
  String get audioDeviceUnavailable => 'Audio Device Unavailable';

  @override
  String get audioOutput => 'Audio Output';

  @override
  String get automatic => 'Automatic';

  @override
  String get availabilityMode => 'Availability Mode';

  @override
  String get batteryAvailability => 'Battery Availability';

  @override
  String get batteryObservation => 'Battery Observation';

  @override
  String get batterySaver => 'Battery Saver';

  @override
  String get batterySettingsDescription => 'Battery Settings Description';

  @override
  String get batteryTab => 'Battery Tab';

  @override
  String get blockContact => 'Zablokuj kontakt';

  @override
  String get blockContactDescription =>
      'Torca zamknie połączenie z tym kontaktem i nie połączy się ponownie, dopóki go nie odblokujesz.';

  @override
  String blockContactTitle(Object name) {
    return 'Zablokowac kontakt $name?';
  }

  @override
  String get blocked => 'Zablokowany';

  @override
  String get blockedSendBlocked => 'Blocked Send Blocked';

  @override
  String get bookmarkMessage => 'Bookmark Message';

  @override
  String bootstrapAttempt(Object attempt, Object label) {
    return '$label $attempt';
  }

  @override
  String bootstrapProgress(Object elapsed, Object ready, Object total) {
    return '$ready $total $elapsed';
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
  String buildTooltip(Object build, Object service) {
    return 'Build $build\\nUsluga providera: $service';
  }

  @override
  String get cancel => 'Anuluj';

  @override
  String get cancelInvitation => 'Anuluj zaproszenie';

  @override
  String get cancelMessage => 'Cancel Message';

  @override
  String get cancelRequest => 'Anuluj zadanie';

  @override
  String get cancelled => 'Anulowano';

  @override
  String get chats => 'Chats';

  @override
  String get checkingInvitation => 'Sprawdzanie zaproszenia...';

  @override
  String get chooseConversation => 'Choose Conversation';

  @override
  String get chooseLanguage => 'Wybierz język';

  @override
  String get chooseLanguagePolish => 'Choose Language Polish';

  @override
  String get chooseNickname => 'Wybierz pseudonim';

  @override
  String get clearConversationHistory => 'Clear Conversation History';

  @override
  String get clearSearch => 'Clear Search';

  @override
  String get close => 'Zamknij';

  @override
  String get closeInvitationDescription => 'Close Invitation Description';

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
  String get collapseNavigation => 'Collapse Navigation';

  @override
  String get comfortableDensity => 'Gestosc wygodna';

  @override
  String get communicationProvider => 'Communication Provider';

  @override
  String get communicationState => 'Communication State';

  @override
  String get compactDensity => 'Gestosc kompaktowa';

  @override
  String get completedTransfers => 'Completed Transfers';

  @override
  String get connecting => 'Laczenie';

  @override
  String connectingPeerThrough(Object provider) {
    return '$provider';
  }

  @override
  String get connection => 'Połączenie';

  @override
  String get connectionDetails => 'Szczegóły połączenia';

  @override
  String get connectionDetailsTitle => 'Szczegoly polaczenia';

  @override
  String connectionEvidenceNote(Object provider) {
    return '$provider';
  }

  @override
  String connectionQuality(Object quality, Object rtt) {
    return '$quality $rtt';
  }

  @override
  String get connectionSelfTest => 'Test polaczenia';

  @override
  String get consecutiveFailures => 'Kolejne bledy';

  @override
  String contactAcceptedJoin(Object name) {
    return '$name zaakceptowal(a) zaproszenie';
  }

  @override
  String get contactActions => 'Akcje kontaktu';

  @override
  String contactAddedToContacts(Object name) {
    return '$name dodano do kontakt�w';
  }

  @override
  String get contactBlocked => 'Kontakt jest zablokowany';

  @override
  String get contactConnected => 'Contact Connected';

  @override
  String get contactConnectedDescription => 'Contact Connected Description';

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
  String get contract => 'Contract';

  @override
  String get contractDecodeFailed =>
      'Klient i runtime używają niezgodnych danych. Zbuduj i wdroż oba ponownie.';

  @override
  String get contractSnapshotReadable => 'Snapshot kontraktu czytelny';

  @override
  String get conversationActions => 'Conversation Actions';

  @override
  String get copy => 'Kopiuj';

  @override
  String get copyCode => 'Kopiuj zaproszenie';

  @override
  String get copyFingerprint => 'Copy Fingerprint';

  @override
  String get couldNotBlockContact => 'Nie udało się zablokować kontaktu';

  @override
  String get couldNotForwardMessage => 'Could Not Forward Message';

  @override
  String get couldNotQueueAttachment =>
      'Nie udalo sie dodac zalacznika do kolejki';

  @override
  String get couldNotRemoveContact => 'Nie udało się usunąć kontaktu';

  @override
  String get couldNotRenameContact => 'Nie udało się zmienić nazwy kontaktu';

  @override
  String get couldNotSaveNickname => 'Could Not Save Nickname';

  @override
  String couldNotStartConversation(Object name) {
    return '$name';
  }

  @override
  String get couldNotStartRadio => 'Could Not Start Radio';

  @override
  String get couldNotUnblockContact => 'Nie udało się odblokować kontaktu';

  @override
  String get couldNotUpdateRadio => 'Could Not Update Radio';

  @override
  String get couldNotUpdateReaction => 'Could Not Update Reaction';

  @override
  String get country => 'Country';

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
    return '$name';
  }

  @override
  String get deleteMessage => 'Delete Message';

  @override
  String get deleteMessageTitle => 'Delete Message Title';

  @override
  String get delivered => 'Dostarczono';

  @override
  String get deliveryFailed => 'Dostarczenie nieudane';

  @override
  String get desktop => 'Pulpit';

  @override
  String deviceFingerprint(Object fingerprint) {
    return '$fingerprint';
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
  String directProviderContact(Object provider) {
    return '$provider';
  }

  @override
  String get displayName => 'Nazwa wyświetlana';

  @override
  String get documentTransfers => 'Document Transfers';

  @override
  String get done => 'Gotowe';

  @override
  String get draft => 'Draft';

  @override
  String get editMessage => 'Edit Message';

  @override
  String get emoji => 'Emoji';

  @override
  String get enableNotifications => 'Włącz powiadomienia';

  @override
  String get encrypting => 'Szyfrowanie';

  @override
  String get endpoint => 'Endpoint';

  @override
  String get englishCountry => 'English Country';

  @override
  String get enterSixCharacterCode =>
      'Wpisz szescioznakowy kod lub zeskanuj kod QR.';

  @override
  String get excellent => 'Doskonaly';

  @override
  String get expandNavigation => 'Expand Navigation';

  @override
  String get exportDiagnostics => 'Eksportuj diagnostyke';

  @override
  String get exportFailed => 'Eksport nieudany';

  @override
  String get exportTorcaDiagnostics => 'Export Torca Diagnostics';

  @override
  String get fair => 'Sredni';

  @override
  String get fileTransfers => 'File Transfers';

  @override
  String get finalizingContact => 'Finalizing Contact';

  @override
  String get fingerprint => 'Fingerprint';

  @override
  String get fingerprintCopied => 'Fingerprint Copied';

  @override
  String get focusedOnly => 'Focused Only';

  @override
  String get followSystem => 'Follow System';

  @override
  String get forwardMessage => 'Forward Message';

  @override
  String forwardNoAvailableAttachments(Object count) {
    return '$count';
  }

  @override
  String forwardSkippedAttachments(Object count) {
    return '$count';
  }

  @override
  String get fullAnimation => 'Full Animation';

  @override
  String get generateInvitation => 'Wygeneruj zaproszenie';

  @override
  String get generatingInvitation => 'Generowanie…';

  @override
  String get good => 'Dobry';

  @override
  String get holdToRecordVoiceClip => 'Hold To Record Voice Clip';

  @override
  String get identicalDeadlineReplacements => 'Identical Deadline Replacements';

  @override
  String get identity => 'Identity';

  @override
  String get identityChanged =>
      'Tożsamość kontaktu uległa zmianie. Sprawdź numer bezpieczeństwa.';

  @override
  String get identityChangedSendBlocked => 'Identity Changed Send Blocked';

  @override
  String get incidentDescription => 'Incident Description';

  @override
  String get incidentSnapshotSaved => 'Incident Snapshot Saved';

  @override
  String get incidentTab => 'Incident Tab';

  @override
  String get incidentTools => 'Incident Tools';

  @override
  String get incomingMessage => 'Wiadomość przychodząca';

  @override
  String get incompatibleStorageEpoch => 'Incompatible Storage Epoch';

  @override
  String get instantMode => 'Instant Mode';

  @override
  String get instantModeEnabled => 'Instant Mode Enabled';

  @override
  String get invalidInput => 'Podana wartość jest nieprawidłowa.';

  @override
  String get invitationCode => 'Kod zaproszenia';

  @override
  String get invitationCodeCopied => 'Pelne zaproszenie skopiowane';

  @override
  String invitationCodeLabel(Object code) {
    return '$code';
  }

  @override
  String invitationExpiresIn(Object countdown) {
    return '$countdown';
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
  String get joinRequestWaiting => 'Join Request Waiting';

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
    return '$time';
  }

  @override
  String get lastSuccessfulProbe => 'Ostatnia udana sonda';

  @override
  String get leaseReasons => 'Lease Reasons';

  @override
  String get light => 'Jasny';

  @override
  String get loadCurrentRunLogs => 'Load Current Run Logs';

  @override
  String get loaded => 'Zaladowano';

  @override
  String get localIdentity => 'Tożsamość lokalna';

  @override
  String get localIdentityCheck => 'Lokalna tozsamosc';

  @override
  String get localIdentityNotReady => 'Local Identity Not Ready';

  @override
  String get localName => 'Nazwa lokalna';

  @override
  String get logsTab => 'Logs Tab';

  @override
  String get markConversationRead => 'Mark Conversation Read';

  @override
  String get markIncident => 'Mark Incident';

  @override
  String get mediaTransfers => 'Media Transfers';

  @override
  String get message => 'Wiadomość';

  @override
  String get messageActions => 'Message Actions';

  @override
  String get messageCancelled => 'Message Cancelled';

  @override
  String get messageCopied => 'Wiadomosc skopiowana';

  @override
  String get messageDeleted => 'Message Deleted';

  @override
  String get messageDetails => 'Szczegoly wiadomosci';

  @override
  String get messageEdited => 'Message Edited';

  @override
  String get messageForwarded => 'Message Forwarded';

  @override
  String get messageQueued =>
      'W kolejce — oczekiwanie na bezpośrednie połączenie';

  @override
  String get messageSenderContact => 'Kontakt';

  @override
  String get messageSenderYou => 'Ty';

  @override
  String messageTooLong(Object maximum) {
    return '$maximum';
  }

  @override
  String get meteredTransfers => 'Metered Transfers';

  @override
  String get microphone => 'Microphone';

  @override
  String get microphonePermissionRequired => 'Microphone Permission Required';

  @override
  String get modern => 'Nowoczesny';

  @override
  String get muteConversation => 'Mute Conversation';

  @override
  String get nativeBridge => 'Most natywny';

  @override
  String get nativeLogTails => 'Native Log Tails';

  @override
  String get nativeLogTailsDescription => 'Native Log Tails Description';

  @override
  String get networkUnavailable =>
      'Wybrane polaczenie komunikacyjne jest obecnie niedostepne.';

  @override
  String get never => 'Nigdy';

  @override
  String get newContact => 'New Contact';

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
  String get nextDeadline => 'Next Deadline';

  @override
  String get nickname => 'Pseudonim';

  @override
  String get nicknameIntro => 'Nickname Intro';

  @override
  String get nicknameRequired => 'Nickname Required';

  @override
  String get noActiveTransfers => 'No Active Transfers';

  @override
  String get noChatsMatch => 'No Chats Match';

  @override
  String get noContactsPaired => 'Brak sparowanych kontaktow';

  @override
  String get noContactsYet => 'Brak kontaktow';

  @override
  String get noForwardableContent => 'No Forwardable Content';

  @override
  String get noInvitations => 'Brak zaproszen';

  @override
  String get noMatchingMessages => 'No Matching Messages';

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
  String get observationRecording => 'Observation Recording';

  @override
  String get observationRecordingDescription =>
      'Observation Recording Description';

  @override
  String get observationState => 'Observation State';

  @override
  String get observationStopped => 'Observation Stopped';

  @override
  String get observationStoppedDescription => 'Observation Stopped Description';

  @override
  String get observationWork => 'Observation Work';

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
  String get originalMessageUnavailable => 'Original Message Unavailable';

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
  String get pairingCompletedMessage => 'Pairing Completed Message';

  @override
  String get pairingExpired => 'Zaproszenie wygasło.';

  @override
  String get pairingInactiveMessage => 'Pairing Inactive Message';

  @override
  String get pairingProviderMismatch =>
      'To zaproszenie pochodzi od innego providera komunikacji.';

  @override
  String get pairingQrSemanticLabel => 'Pairing Qr Semantic Label';

  @override
  String get pairingRequestDescription =>
      'To urządzenie dołączyło do Twojego zaproszenia. Sprawdź dane kontaktu przed akceptacją.';

  @override
  String pairingStateLabel(Object state) {
    return '$state';
  }

  @override
  String get pauseAll => 'Pause All';

  @override
  String get pauseLarge => 'Pause Large';

  @override
  String get peerOffline => 'Kontakt offline';

  @override
  String get peerState => 'Peer State';

  @override
  String get pendingOperations => 'Pending Operations';

  @override
  String get pinConversation => 'Pin Conversation';

  @override
  String get playVoiceMessage => 'Play Voice Message';

  @override
  String get polishCountry => 'Polish Country';

  @override
  String get poor => 'Slaby';

  @override
  String get preparingDownload => 'Przygotowanie pobierania';

  @override
  String get preparingPrivateSpace => 'Preparing Private Space';

  @override
  String get preparingPrivateSpaceDescription =>
      'Preparing Private Space Description';

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
  String get productVersion => 'Product Version';

  @override
  String get profileNotReady => 'Bezpieczny profil nie jest jeszcze gotowy.';

  @override
  String get providerEndpoint => 'Endpoint providera';

  @override
  String get providerEndpointAvailable => 'Provider Endpoint Available';

  @override
  String get providerEndpointUnavailable => 'Provider Endpoint Unavailable';

  @override
  String providerName(Object provider) {
    return '$provider';
  }

  @override
  String providerReady(Object provider) {
    return '$provider';
  }

  @override
  String providerReconnecting(Object provider) {
    return '$provider';
  }

  @override
  String providerStarting(Object provider) {
    return '$provider';
  }

  @override
  String providerStateLabel(Object provider, Object state) {
    return '$provider $state';
  }

  @override
  String get published => 'Opublikowano';

  @override
  String get quality => 'Jakość';

  @override
  String get queued => 'Queued';

  @override
  String get radioChannelInterrupted => 'Radio Channel Interrupted';

  @override
  String get radioChannelReady => 'Radio Channel Ready';

  @override
  String get radioChannelRestored => 'Radio Channel Restored';

  @override
  String get radioConnecting => 'Radio Connecting';

  @override
  String radioDisabledBy(Object actor) {
    return '$actor';
  }

  @override
  String radioEnabledBy(Object actor) {
    return '$actor';
  }

  @override
  String get radioMode => 'Radio Mode';

  @override
  String get radioModeDescription => 'Radio Mode Description';

  @override
  String get radioReady => 'Radio Ready';

  @override
  String radioReceiving(Object name) {
    return '$name';
  }

  @override
  String get radioReconnecting => 'Radio Reconnecting';

  @override
  String get radioRequestingFloor => 'Radio Requesting Floor';

  @override
  String get radioTransmitting => 'Radio Transmitting';

  @override
  String radioTransportFailure(Object code) {
    return '$code';
  }

  @override
  String get radioUnavailable => 'Radio Unavailable';

  @override
  String get radioWaitingForPeer => 'Radio Waiting For Peer';

  @override
  String get rawDiagnostics => 'Surowa diagnostyka';

  @override
  String get reactToMessage => 'React To Message';

  @override
  String get read => 'Odczytano';

  @override
  String get receivingSecurely => 'Bezpieczny odbior';

  @override
  String get recentEmoji => 'Recent Emoji';

  @override
  String get recentInvitations => 'Ostatnie zaproszenia';

  @override
  String get reconnectAttempts => 'Proby ponownego polaczenia';

  @override
  String get reconnecting => 'Ponowne laczenie';

  @override
  String reconnectingPeerThrough(Object provider) {
    return '$provider';
  }

  @override
  String get reconnectingShort => 'Laczenie';

  @override
  String get recordingTransfers => 'Recording Transfers';

  @override
  String get redactedDeveloperEventStream =>
      'Zanonimizowany strumien zdarzen deweloperskich';

  @override
  String get redactedHealthEventsReadable =>
      'Zanonimizowane zdarzenia zdrowia czytelne';

  @override
  String get redactedSchedulerDescription => 'Redacted Scheduler Description';

  @override
  String get reduceMotion => 'Ogranicz ruch';

  @override
  String get refresh => 'Odswiez';

  @override
  String get refreshProviderRoute => 'Refresh Provider Route';

  @override
  String get regressionScore => 'Regression Score';

  @override
  String get reject => 'Odrzuc';

  @override
  String remoteIdentity(Object id) {
    return 'Tozsamosc $id';
  }

  @override
  String get remoteIdentityTitle => 'Remote Identity Title';

  @override
  String get remove => 'Usuń';

  @override
  String get removeAttachment => 'Usun zalacznik';

  @override
  String get removeBookmark => 'Remove Bookmark';

  @override
  String get removeContact => 'Usuń kontakt';

  @override
  String get removeContactDescription =>
      'Usuwa to lokalną relację, historię rozmowy, oczekujące operacje i chronione dane uwierzytelniające kontaktu.';

  @override
  String removeContactTitle(Object name) {
    return 'Usunac kontakt $name?';
  }

  @override
  String get renameContact => 'Zmień nazwę kontaktu';

  @override
  String get reply => 'Odpowiedź';

  @override
  String get resetBaseline => 'Reset Baseline';

  @override
  String get resetVerification => 'Reset Verification';

  @override
  String get restartApplication => 'Restart Application';

  @override
  String get restoreConversation => 'Restore Conversation';

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
  String get route => 'Route';

  @override
  String get routeRefreshRequested => 'Route Refresh Requested';

  @override
  String get routeRefreshRequired => 'Route Refresh Required';

  @override
  String get runSelfTest => 'Uruchom test';

  @override
  String get runtimeHealth => 'Runtime Health';

  @override
  String runtimeNotReadyDiagnostic(Object provider) {
    return '$provider';
  }

  @override
  String get runtimePreparationFailed =>
      'Nie udalo sie przygotowac lokalnego szyfrowanego runtime. Tozsamosc nie zostala zmieniona.';

  @override
  String get runtimeTab => 'Runtime Tab';

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
  String get saving => 'Saving';

  @override
  String get scanQr => 'Skanuj QR';

  @override
  String get scheduledWork => 'Scheduled Work';

  @override
  String get searchChats => 'Search Chats';

  @override
  String get searchConversationHint => 'Search Conversation Hint';

  @override
  String get searchMessages => 'Szukaj wiadomosci';

  @override
  String searchResultsCount(Object count) {
    return '$count';
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
  String get sharedMedia => 'Shared Media';

  @override
  String sharedMediaCount(Object count) {
    return '$count';
  }

  @override
  String get sourceCommit => 'Source Commit';

  @override
  String get startConversation => 'Rozpocznij rozmowę';

  @override
  String get startObservation => 'Start Observation';

  @override
  String get startingSecureNetwork => 'Uruchamianie bezpiecznej sieci…';

  @override
  String get startingShort => 'Start';

  @override
  String get state => 'Stan';

  @override
  String get staticIdle => 'Static Idle';

  @override
  String get status => 'Stan';

  @override
  String get stopObservation => 'Stop Observation';

  @override
  String get storageEpoch => 'Storage Epoch';

  @override
  String get storageFailure =>
      'Nie udało się zakończyć operacji na szyfrowanym magazynie.';

  @override
  String get system => 'System';

  @override
  String get systemDefaultAudioDevice => 'System Default Audio Device';

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
  String get transfers => 'Transfers';

  @override
  String get transport => 'Transport';

  @override
  String get typeToSearchConversation => 'Type To Search Conversation';

  @override
  String get unavailable => 'Niedostępne';

  @override
  String get unblockContact => 'Odblokuj kontakt';

  @override
  String get unknown => 'Nieznany';

  @override
  String get unknownCountry => 'Unknown Country';

  @override
  String get unmuteConversation => 'Unmute Conversation';

  @override
  String get unpinConversation => 'Unpin Conversation';

  @override
  String get unverified => 'Unverified';

  @override
  String get variant => 'Variant';

  @override
  String get verification => 'Verification';

  @override
  String get verified => 'Verified';

  @override
  String get verifiedOnDevice => 'Zweryfikowano na urzadzeniu';

  @override
  String get verifyContact => 'Verify Contact';

  @override
  String get verifyFingerprintBeforeAccepting =>
      'Verify Fingerprint Before Accepting';

  @override
  String get visualActivity => 'Visual Activity';

  @override
  String voiceClipRecording(Object secondsLeft) {
    return '$secondsLeft';
  }

  @override
  String get voiceClipRecordingFailed => 'Voice Clip Recording Failed';

  @override
  String get voiceMessage => 'Voice Message';

  @override
  String get voiceMessagePlayed => 'Voice Message Played';

  @override
  String get voiceMessageReady => 'Voice Message Ready';

  @override
  String waitingForDependency(Object dependency) {
    return '$dependency';
  }

  @override
  String get waitingForPeer => 'Oczekiwanie na kontakt';

  @override
  String get waitingToReceive => 'Oczekiwanie na odbior';

  @override
  String get wakeSources => 'Wake Sources';

  @override
  String get whyAwake => 'Why Awake';

  @override
  String get yesterday => 'Wczoraj';

  @override
  String get yourIdentity => 'Twoja tożsamość';

  @override
  String get yourInvitation => 'Twoje zaproszenie';

  @override
  String get zeroDelayDeadlines => 'Zero Delay Deadlines';
}

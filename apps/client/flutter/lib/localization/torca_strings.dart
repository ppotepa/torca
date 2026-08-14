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
  String get senderYou => _pl ? 'Ty' : 'You';
  String get senderContact => _pl ? 'Kontakt' : 'Contact';
  String get outgoingMessage =>
      _pl ? 'Wiadomość wychodząca' : 'Outgoing message';
  String get incomingMessage =>
      _pl ? 'Wiadomość przychodząca' : 'Incoming message';
  String get sent => _pl ? 'Wysłano' : 'Sent';
  String get delivered => _pl ? 'Dostarczono' : 'Delivered';
  String get read => _pl ? 'Odczytano' : 'Read';
  String get messageQueued => _pl
      ? 'W kolejce — oczekiwanie na bezpośrednie połączenie'
      : 'Queued — waiting for a direct peer connection';
  String get deliveryFailed =>
      _pl ? 'Dostarczenie nieudane' : 'Delivery failed';
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
      ? 'Bezpieczna sieć Tor jest gotowa. Ta nazwa będzie widoczna dla kontaktów.'
      : 'The secure Tor network is ready. This name will be shown to contacts.';
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
  String get relayNotReady => _pl
      ? 'Parowanie będzie dostępne, gdy bezpieczny relay będzie gotowy.'
      : 'Pairing is unavailable until the secure relay is ready.';
  String get relayDegraded => _pl
      ? 'Relay ponownie nawiązuje połączenie. Spróbuj za chwilę.'
      : 'The relay is reconnecting. Try again shortly.';
  String get profileNotReady => _pl
      ? 'Bezpieczny profil nie jest jeszcze gotowy.'
      : 'The secure profile is not ready yet.';
  String get identityChanged => _pl
      ? 'Tożsamość kontaktu uległa zmianie. Sprawdź numer bezpieczeństwa.'
      : 'The contact identity changed. Verify the Safety Number.';
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
      ? 'Bezpieczne połączenie Tor jest obecnie niedostępne.'
      : 'The secure Tor connection is currently unavailable.';
  String get runtimeUnavailable => _pl
      ? 'Bezpieczny runtime Torca jest obecnie niedostępny.'
      : 'The secure Torca runtime is currently unavailable.';
  String get contractDecodeFailed => _pl
      ? 'Klient i runtime używają niezgodnych danych. Zbuduj i wdroż oba ponownie.'
      : 'The client and native runtime use incompatible data. Rebuild and redeploy both.';
  String get operationFailed => _pl
      ? 'Nie udało się wykonać operacji.'
      : 'The operation could not be completed.';
  String get yourIdentity => _pl ? 'Twoja tożsamość' : 'Your identity';
  String get localIdentity => _pl ? 'Tożsamość lokalna' : 'Local identity';
  String get displayName => _pl ? 'Nazwa wyświetlana' : 'Display name';
  String get torState => _pl ? 'Stan Tor' : 'Tor state';
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
  String get noActiveTransfers =>
      _pl ? 'Brak aktywnych transferow.' : 'No active transfers.';
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
  String get microphonePermissionRequired => _pl
      ? 'Dostep do mikrofonu jest wymagany do nadawania.'
      : 'Microphone access is required to transmit.';
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
  String buildTooltip(String build, String relay) => _pl
      ? 'Build $build\nWersja relay: $relay'
      : 'Torca build $build\nRelay version: $relay';
  String buildLabel(String build) => _pl ? 'build $build' : 'build $build';
  String get secureRuntimeNotReady => _pl
      ? 'Bezpieczne srodowisko nie jest gotowe'
      : 'Secure runtime is not ready';
  String get runtimePreparationFailed => _pl
      ? 'Nie udalo sie przygotowac lokalnego szyfrowanego srodowiska. Tozsamosc nie zostala zmieniona.'
      : 'Torca could not prepare the local encrypted runtime. Your identity has not been changed.';
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
  String get directP2pOverTor => 'Direct P2P over Tor';
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
  String get invitationJoinSent => _pl
      ? 'Zadanie dolaczenia wyslane. Otrzymasz powiadomienie po akceptacji.'
      : 'Join request sent. You will be notified when it is accepted.';
  String get invitationSavedLocally => _pl
      ? 'Zapisano lokalnie. Zadanie zostanie wyslane, gdy endpoint bedzie gotowy.'
      : 'Saved locally. It will be sent when your private endpoint is ready.';
  String get openConversation => _pl ? 'Otworz rozmowe' : 'Open conversation';
  String get noMessagesYetDescription => _pl
      ? 'Wiadomosci sa wysylane bezposrednio przez Tor.'
      : 'Messages are sent directly through Tor.';
  String get attachmentSyncing =>
      _pl ? 'Synchronizacja zalacznika…' : 'Attachment is syncing…';
  String get closeSearch => _pl ? 'Zamknij wyszukiwanie' : 'Close search';
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
  String get copyCode => _pl ? 'Kopiuj kod' : 'Copy code';
  String get invitationCodeCopied =>
      _pl ? 'Kod zaproszenia skopiowany' : 'Invitation code copied';
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
  String get refresh => _pl ? 'Odswiez' : 'Refresh';
  String get removeAttachment => _pl ? 'Usun zalacznik' : 'Remove attachment';
  String get scanQr => _pl ? 'Skanuj QR' : 'Scan QR';
  String get contactDetails => _pl ? 'Szczegoly kontaktu' : 'Contact details';
  String get contactBlocked =>
      _pl ? 'Kontakt jest zablokowany' : 'Contact is blocked';
  String get connecting => _pl ? 'Laczenie' : 'Connecting';
  String get reconnecting => _pl ? 'Ponowne laczenie' : 'Reconnecting';
  String get peerOffline => _pl ? 'Kontakt offline' : 'Peer is offline';
  String get connectingPeerThroughTor =>
      _pl ? 'Laczenie z kontaktem przez Tor' : 'Connecting to peer through Tor';
  String get reconnectingPeerThroughTor => _pl
      ? 'Ponowne laczenie z kontaktem przez Tor'
      : 'Reconnecting to peer through Tor';
  String get torReady => _pl ? 'Tor gotowy' : 'Tor ready';
  String get torStarting => _pl ? 'Tor uruchamia sie' : 'Tor starting';
  String get torReconnecting =>
      _pl ? 'Tor ponownie sie laczy' : 'Tor reconnecting';
  String torStateLabel(String state) =>
      'Tor: ${state.isEmpty ? 'offline' : state}';
  String get p2pShort => 'P2P';
  String get torShort => 'Tor';
  String get startingShort => _pl ? 'Start' : 'Starting';
  String get reconnectingShort => _pl ? 'Laczenie' : 'Reconnecting';
  String get offlineShort => 'Offline';
  String get nativeBridge => _pl ? 'Most natywny' : 'Native bridge';
  String get localIdentityCheck => _pl ? 'Lokalna tozsamosc' : 'Local identity';
  String get onionService => _pl ? 'Usluga onion' : 'Onion service';
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
  String get noOnionAddress => _pl ? 'Brak adresu onion' : 'No onion address';
  String get redactedHealthEventsReadable => _pl
      ? 'Zanonimizowane zdarzenia zdrowia czytelne'
      : 'Redacted health events readable';
  String get noReadableHealthEvents =>
      _pl ? 'Brak czytelnych zdarzen zdrowia' : 'No readable health events';
  String get startingSecureNetwork =>
      _pl ? 'Uruchamianie bezpiecznej sieci…' : 'Starting secure network…';
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

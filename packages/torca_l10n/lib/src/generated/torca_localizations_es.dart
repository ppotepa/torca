// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'torca_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class TorcaLocalizationsEs extends TorcaLocalizations {
  TorcaLocalizationsEs([String locale = 'es']) : super(locale);

  @override
  String get aboutTorca => 'About Torca';

  @override
  String get accept => 'Accept';

  @override
  String get activeDemands => 'Active Demands';

  @override
  String get activeInvitationsDescription =>
      'Your active invitations and pairing requests will appear here.';

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
  String get appearance => 'Appearance';

  @override
  String get appearanceTitle => 'Apariencia';

  @override
  String get applicationMenu => 'Application menu';

  @override
  String get archiveConversation => 'Archive Conversation';

  @override
  String get attachFiles => 'Attach files';

  @override
  String get attachmentAckTimeout => 'waiting for peer acknowledgement';

  @override
  String get attachmentDependencyMissing => 'waiting for conversation';

  @override
  String get attachmentIntegrityFailed => 'integrity check failed';

  @override
  String get attachmentMessagePending => 'waiting for message';

  @override
  String get attachmentOperationFailed => 'Attachment operation failed';

  @override
  String get attachmentPeerUnavailable => 'peer unavailable';

  @override
  String get attachmentRetryAvailable => 'retry available';

  @override
  String get attachmentSaved => 'Attachment saved';

  @override
  String get attachmentStorageFailed => 'local storage failed';

  @override
  String get attachmentSyncing => 'Attachment is syncing…';

  @override
  String attachmentsQueued(num count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count attachments queued',
      one: '1 attachment queued',
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
  String get blockContact => 'Block contact';

  @override
  String get blockContactDescription =>
      'Torca will close the peer connection and will not reconnect until you unblock this contact.';

  @override
  String blockContactTitle(Object name) {
    return 'Block $name?';
  }

  @override
  String get blocked => 'Blocked';

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
  String get buildAndConnectionInfo => 'Build & connection info';

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
    return 'Torca build $build\\nProvider service: $service';
  }

  @override
  String get cancel => 'Cancelar';

  @override
  String get cancelInvitation => 'Cancel invitation';

  @override
  String get cancelMessage => 'Cancel Message';

  @override
  String get cancelRequest => 'Cancel request';

  @override
  String get cancelled => 'Cancelled';

  @override
  String get chats => 'Chats';

  @override
  String get checkingInvitation => 'Checking invitation...';

  @override
  String get chooseConversation => 'Choose Conversation';

  @override
  String get chooseLanguage => 'Elige tu idioma';

  @override
  String get chooseLanguagePolish => 'Choose Language Polish';

  @override
  String get chooseNickname => 'Elige tu apodo';

  @override
  String get clearConversationHistory => 'Clear Conversation History';

  @override
  String get clearSearch => 'Clear Search';

  @override
  String get close => 'Cerrar';

  @override
  String get closeInvitationDescription => 'Close Invitation Description';

  @override
  String get closeScanner => 'Close scanner';

  @override
  String get closeSearch => 'Close search';

  @override
  String get closeToTray => 'Cerrar en la bandeja';

  @override
  String get closeToTrayDescription =>
      'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.';

  @override
  String get closeTooltip => 'Close';

  @override
  String get collapseNavigation => 'Collapse Navigation';

  @override
  String get comfortableDensity => 'Comfortable density';

  @override
  String get communicationProvider => 'Communication Provider';

  @override
  String get communicationState => 'Communication State';

  @override
  String get compactDensity => 'Compact density';

  @override
  String get completedTransfers => 'Completed Transfers';

  @override
  String get connecting => 'Connecting';

  @override
  String connectingPeerThrough(Object provider) {
    return '$provider';
  }

  @override
  String get connection => 'Connection';

  @override
  String get connectionDetails => 'Connection details';

  @override
  String get connectionDetailsTitle => 'Connection details';

  @override
  String connectionEvidenceNote(Object provider) {
    return '$provider';
  }

  @override
  String connectionQuality(Object quality, Object rtt) {
    return '$quality $rtt';
  }

  @override
  String get connectionSelfTest => 'Connection self-test';

  @override
  String get consecutiveFailures => 'Consecutive failures';

  @override
  String contactAcceptedJoin(Object name) {
    return '$name accepted your join request';
  }

  @override
  String get contactActions => 'Contact actions';

  @override
  String contactAddedToContacts(Object name) {
    return '$name was added to Contacts';
  }

  @override
  String get contactBlocked => 'Contact is blocked';

  @override
  String get contactConnected => 'Contact Connected';

  @override
  String get contactConnectedDescription => 'Contact Connected Description';

  @override
  String get contactDetails => 'Contact details';

  @override
  String get contactInformation => 'Contact information';

  @override
  String get contactLabel => 'Contact';

  @override
  String get contactUnavailable => 'This contact is no longer available.';

  @override
  String get contacts => 'Contacts';

  @override
  String contactsCount(num count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count private contacts',
      one: '1 private contact',
    );
    return '$_temp0';
  }

  @override
  String get continueLabel => 'Continuar';

  @override
  String get contract => 'Contract';

  @override
  String get contractDecodeFailed =>
      'The client and native runtime use incompatible data. Rebuild and redeploy both.';

  @override
  String get contractSnapshotReadable => 'Contract snapshot readable';

  @override
  String get conversationActions => 'Conversation Actions';

  @override
  String get copy => 'Copy';

  @override
  String get copyCode => 'Copy invitation';

  @override
  String get copyFingerprint => 'Copy Fingerprint';

  @override
  String get couldNotBlockContact => 'Could not block contact';

  @override
  String get couldNotForwardMessage => 'Could Not Forward Message';

  @override
  String get couldNotQueueAttachment => 'Could not queue attachment';

  @override
  String get couldNotRemoveContact => 'Could not remove contact';

  @override
  String get couldNotRenameContact => 'Could not rename contact';

  @override
  String get couldNotSaveNickname => 'Could Not Save Nickname';

  @override
  String couldNotStartConversation(Object name) {
    return '$name';
  }

  @override
  String get couldNotStartRadio => 'Could Not Start Radio';

  @override
  String get couldNotUnblockContact => 'Could not unblock contact';

  @override
  String get couldNotUpdateRadio => 'Could Not Update Radio';

  @override
  String get couldNotUpdateReaction => 'Could Not Update Reaction';

  @override
  String get country => 'Country';

  @override
  String get createInvitationForContact =>
      'Create an invitation to add a private contact.';

  @override
  String get createManageInvitations =>
      'Create and manage short-lived private contact invitations.';

  @override
  String get createdInvitation => 'Created invitation';

  @override
  String get dark => 'Dark';

  @override
  String defaultAudioDevice(Object name) {
    return '$name';
  }

  @override
  String get deleteMessage => 'Delete Message';

  @override
  String get deleteMessageTitle => 'Delete Message Title';

  @override
  String get delivered => 'Delivered';

  @override
  String get deliveryFailed => 'Delivery failed';

  @override
  String get desktop => 'Desktop';

  @override
  String deviceFingerprint(Object fingerprint) {
    return '$fingerprint';
  }

  @override
  String get diagnostics => 'Diagnostics';

  @override
  String get diagnosticsExported => 'Diagnostics exported';

  @override
  String get diagnosticsStream => 'Diagnostics stream';

  @override
  String directPeerLinksReady(Object ready, Object total) {
    return '$ready of $total direct peer links ready';
  }

  @override
  String get directPeers => 'Direct peers';

  @override
  String directProviderContact(Object provider) {
    return '$provider';
  }

  @override
  String get displayName => 'Display name';

  @override
  String get documentTransfers => 'Document Transfers';

  @override
  String get done => 'Done';

  @override
  String get draft => 'Draft';

  @override
  String get editMessage => 'Edit Message';

  @override
  String get emoji => 'Emoji';

  @override
  String get enableNotifications => 'Activar notificaciones';

  @override
  String get encrypting => 'Encrypting';

  @override
  String get endpoint => 'Endpoint';

  @override
  String get englishCountry => 'English Country';

  @override
  String get enterSixCharacterCode =>
      'Enter a six-character code or scan the QR code.';

  @override
  String get excellent => 'Excellent';

  @override
  String get expandNavigation => 'Expand Navigation';

  @override
  String get exportDiagnostics => 'Export diagnostics';

  @override
  String get exportFailed => 'Export failed';

  @override
  String get exportTorcaDiagnostics => 'Export Torca Diagnostics';

  @override
  String get fair => 'Fair';

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
  String get generateInvitation => 'Generate Invitation';

  @override
  String get generatingInvitation => 'Generating…';

  @override
  String get good => 'Good';

  @override
  String get holdToRecordVoiceClip => 'Hold To Record Voice Clip';

  @override
  String get identicalDeadlineReplacements => 'Identical Deadline Replacements';

  @override
  String get identity => 'Identity';

  @override
  String get identityChanged =>
      'The contact identity changed. Verify the Safety Number.';

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
  String get incomingMessage => 'Incoming message';

  @override
  String get incompatibleStorageEpoch => 'Incompatible Storage Epoch';

  @override
  String get instantMode => 'Instant Mode';

  @override
  String get instantModeEnabled => 'Instant Mode Enabled';

  @override
  String get invalidInput => 'The supplied value is not valid.';

  @override
  String get invitationCode => 'Invitation code';

  @override
  String get invitationCodeCopied => 'Full invitation copied';

  @override
  String invitationCodeLabel(Object code) {
    return '$code';
  }

  @override
  String invitationExpiresIn(Object countdown) {
    return '$countdown';
  }

  @override
  String get invitationGenerating => 'Generating a private invitation...';

  @override
  String get invitationJoinSent =>
      'Join request sent. You will be notified when it is accepted.';

  @override
  String get invitationOperationFailed => 'Invitation operation failed';

  @override
  String get invitationQueued => 'Invitation queued for the secure network.';

  @override
  String get invitationSavedLocally =>
      'Saved locally. It will retry when the selected communication provider is ready.';

  @override
  String get invitationWaitingForNetwork =>
      'Invitation is waiting for the network.';

  @override
  String get invitations => 'Invitations';

  @override
  String get itemAlreadyExists => 'This item already exists.';

  @override
  String get itemNotFound => 'The item is no longer available.';

  @override
  String get joinInvitation => 'Join invitation';

  @override
  String get joinRequestWaiting => 'Join Request Waiting';

  @override
  String get joinedInvitation => 'Joined invitation';

  @override
  String get jumpToLatest => 'Jump to latest message';

  @override
  String get language => 'Language';

  @override
  String get languageEnglish => 'English';

  @override
  String get languagePolish => 'Polish';

  @override
  String get languageSystem => 'System language';

  @override
  String get languageTitle => 'Idioma';

  @override
  String get lastSeen => 'Last seen';

  @override
  String lastSeenAt(Object time) {
    return '$time';
  }

  @override
  String get lastSuccessfulProbe => 'Last successful probe';

  @override
  String get leaseReasons => 'Lease Reasons';

  @override
  String get light => 'Light';

  @override
  String get loadCurrentRunLogs => 'Load Current Run Logs';

  @override
  String get loaded => 'Loaded';

  @override
  String get localIdentity => 'Local identity';

  @override
  String get localIdentityCheck => 'Local identity';

  @override
  String get localIdentityNotReady => 'Local Identity Not Ready';

  @override
  String get localName => 'Local name';

  @override
  String get logsTab => 'Logs Tab';

  @override
  String get markConversationRead => 'Mark Conversation Read';

  @override
  String get markIncident => 'Mark Incident';

  @override
  String get mediaTransfers => 'Media Transfers';

  @override
  String get message => 'Mensaje';

  @override
  String get messageActions => 'Message Actions';

  @override
  String get messageCancelled => 'Message Cancelled';

  @override
  String get messageCopied => 'Message copied';

  @override
  String get messageDeleted => 'Message Deleted';

  @override
  String get messageDetails => 'Message details';

  @override
  String get messageEdited => 'Message Edited';

  @override
  String get messageForwarded => 'Message Forwarded';

  @override
  String get messageQueued => 'Queued — waiting for a direct peer connection';

  @override
  String get messageSenderContact => 'Contacto';

  @override
  String get messageSenderYou => 'Tú';

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
  String get modern => 'Modern';

  @override
  String get muteConversation => 'Mute Conversation';

  @override
  String get nativeBridge => 'Native bridge';

  @override
  String get nativeLogTails => 'Native Log Tails';

  @override
  String get nativeLogTailsDescription => 'Native Log Tails Description';

  @override
  String get networkUnavailable =>
      'The selected communication connection is currently unavailable.';

  @override
  String get never => 'Never';

  @override
  String get newContact => 'New Contact';

  @override
  String get newDevice => 'New device';

  @override
  String get newMessages => 'New messages';

  @override
  String get newPairing => 'New pairing';

  @override
  String get newPairingRequest => 'New pairing request';

  @override
  String get newPrivateMessage => 'New private message';

  @override
  String get nextDeadline => 'Next Deadline';

  @override
  String get nickname => 'Apodo';

  @override
  String get nicknameIntro => 'Nickname Intro';

  @override
  String get nicknameRequired => 'Nickname Required';

  @override
  String get noActiveTransfers => 'No Active Transfers';

  @override
  String get noChatsMatch => 'No Chats Match';

  @override
  String get noContactsPaired => 'No contacts paired';

  @override
  String get noContactsYet => 'No contacts yet';

  @override
  String get noForwardableContent => 'No Forwardable Content';

  @override
  String get noInvitations => 'No invitations';

  @override
  String get noMatchingMessages => 'No Matching Messages';

  @override
  String get noMessagesYet => 'No messages yet';

  @override
  String get noMessagesYetDescription =>
      'Messages are sent directly through the selected communication provider.';

  @override
  String get noReadableHealthEvents => 'No readable health events';

  @override
  String get notInitialized => 'Not initialized';

  @override
  String get notMeasured => 'Not measured';

  @override
  String get notificationPrivacy =>
      'Show private-message notifications without message content.';

  @override
  String get notifications => 'Notifications';

  @override
  String get notificationsTitle => 'Notificaciones';

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
  String get open => 'Open';

  @override
  String get openChat => 'Open chat';

  @override
  String get openConversation => 'Open conversation';

  @override
  String get operationFailed => 'The operation could not be completed.';

  @override
  String get originalMessageUnavailable => 'Original Message Unavailable';

  @override
  String get outgoingMessage => 'Outgoing message';

  @override
  String get p2pShort => 'P2P';

  @override
  String get pairContact => 'Pair contact';

  @override
  String get pairContactHint => 'Pair a contact to start a conversation.';

  @override
  String get pairingBootstrapRequired =>
      'For this provider, scan the QR code or paste the full invitation link.';

  @override
  String get pairingCompletedMessage => 'Pairing Completed Message';

  @override
  String get pairingExpired => 'The pairing invitation has expired.';

  @override
  String get pairingInactiveMessage => 'Pairing Inactive Message';

  @override
  String get pairingProviderMismatch =>
      'This invitation belongs to a different communication provider.';

  @override
  String get pairingQrSemanticLabel => 'Pairing Qr Semantic Label';

  @override
  String get pairingRequestDescription =>
      'This device joined your invitation. Review the contact details before accepting.';

  @override
  String pairingStateLabel(Object state) {
    return '$state';
  }

  @override
  String get pauseAll => 'Pause All';

  @override
  String get pauseLarge => 'Pause Large';

  @override
  String get peerOffline => 'Peer is offline';

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
  String get poor => 'Poor';

  @override
  String get preparingDownload => 'Preparing download';

  @override
  String get preparingPrivateSpace => 'Preparing Private Space';

  @override
  String get preparingPrivateSpaceDescription =>
      'Preparing Private Space Description';

  @override
  String get preparingSecureCopy => 'Preparing secure copy';

  @override
  String get preparingUpload => 'Preparing upload';

  @override
  String get presence => 'Presence';

  @override
  String get privacy => 'Privacy';

  @override
  String get privacyTitle => 'Privacidad';

  @override
  String get productVersion => 'Product Version';

  @override
  String get profileNotReady => 'The secure profile is not ready yet.';

  @override
  String get providerEndpoint => 'Provider endpoint';

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
  String get published => 'Published';

  @override
  String get quality => 'Quality';

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
  String get rawDiagnostics => 'Raw diagnostics';

  @override
  String get reactToMessage => 'React To Message';

  @override
  String get read => 'Read';

  @override
  String get receivingSecurely => 'Receiving securely';

  @override
  String get recentEmoji => 'Recent Emoji';

  @override
  String get recentInvitations => 'Recent invitations';

  @override
  String get reconnectAttempts => 'Reconnect attempts';

  @override
  String get reconnecting => 'Reconnecting';

  @override
  String reconnectingPeerThrough(Object provider) {
    return '$provider';
  }

  @override
  String get reconnectingShort => 'Reconnecting';

  @override
  String get recordingTransfers => 'Recording Transfers';

  @override
  String get redactedDeveloperEventStream => 'Redacted developer event stream';

  @override
  String get redactedHealthEventsReadable => 'Redacted health events readable';

  @override
  String get redactedSchedulerDescription => 'Redacted Scheduler Description';

  @override
  String get reduceMotion => 'Reduce motion';

  @override
  String get refresh => 'Refresh';

  @override
  String get refreshProviderRoute => 'Refresh Provider Route';

  @override
  String get regressionScore => 'Regression Score';

  @override
  String get reject => 'Reject';

  @override
  String remoteIdentity(Object id) {
    return 'Identity $id';
  }

  @override
  String get remoteIdentityTitle => 'Remote Identity Title';

  @override
  String get remove => 'Remove';

  @override
  String get removeAttachment => 'Remove attachment';

  @override
  String get removeBookmark => 'Remove Bookmark';

  @override
  String get removeContact => 'Remove contact';

  @override
  String get removeContactDescription =>
      'This removes the local relationship, conversation history, pending work and protected peer credential.';

  @override
  String removeContactTitle(Object name) {
    return 'Remove $name?';
  }

  @override
  String get renameContact => 'Rename contact';

  @override
  String get reply => 'Reply';

  @override
  String get resetBaseline => 'Reset Baseline';

  @override
  String get resetVerification => 'Reset Verification';

  @override
  String get restartApplication => 'Restart Application';

  @override
  String get restoreConversation => 'Restore Conversation';

  @override
  String get retry => 'Reintentar';

  @override
  String get retryGeneration => 'Retry generation';

  @override
  String get retryNow => 'Retry now';

  @override
  String get retrying => 'Retrying…';

  @override
  String get roundTrip => 'Round trip';

  @override
  String get route => 'Route';

  @override
  String get routeRefreshRequested => 'Route Refresh Requested';

  @override
  String get routeRefreshRequired => 'Route Refresh Required';

  @override
  String get runSelfTest => 'Run self-test';

  @override
  String get runtimeHealth => 'Runtime Health';

  @override
  String runtimeNotReadyDiagnostic(Object provider) {
    return '$provider';
  }

  @override
  String get runtimePreparationFailed =>
      'Torca could not prepare the local encrypted runtime. Your identity has not been changed.';

  @override
  String get runtimeTab => 'Runtime Tab';

  @override
  String get runtimeUnavailable =>
      'The secure Torca runtime is currently unavailable.';

  @override
  String get sampleContactName => 'Alice';

  @override
  String get sampleOnline => 'online';

  @override
  String get sampleTime => '14:22';

  @override
  String get save => 'Guardar';

  @override
  String get saveAs => 'Save as';

  @override
  String get saveAttachment => 'Save attachment';

  @override
  String get saving => 'Saving';

  @override
  String get scanQr => 'Scan QR';

  @override
  String get scheduledWork => 'Scheduled Work';

  @override
  String get searchChats => 'Search Chats';

  @override
  String get searchConversationHint => 'Search Conversation Hint';

  @override
  String get searchMessages => 'Search messages';

  @override
  String searchResultsCount(Object count) {
    return '$count';
  }

  @override
  String get secureRuntimeNotReady => 'Secure runtime is not ready';

  @override
  String get selectConversation => 'Select a conversation';

  @override
  String get sendMessage => 'Send message';

  @override
  String get sendReadReceipts => 'Send read receipts';

  @override
  String get sendReadReceiptsDescription =>
      'Messages are marked read locally, but contacts see the Read state only when this option is enabled.';

  @override
  String get senderContact => 'Contact';

  @override
  String get senderYou => 'You';

  @override
  String get sendingSecurely => 'Sending securely';

  @override
  String get sent => 'Sent';

  @override
  String get settings => 'Settings';

  @override
  String get settingsTitle => 'Ajustes';

  @override
  String get sharedMedia => 'Shared Media';

  @override
  String sharedMediaCount(Object count) {
    return '$count';
  }

  @override
  String get sourceCommit => 'Source Commit';

  @override
  String get startConversation => 'Start conversation';

  @override
  String get startObservation => 'Start Observation';

  @override
  String get startingSecureNetwork => 'Starting secure network…';

  @override
  String get startingShort => 'Starting';

  @override
  String get state => 'State';

  @override
  String get staticIdle => 'Static Idle';

  @override
  String get status => 'Status';

  @override
  String get stopObservation => 'Stop Observation';

  @override
  String get storageEpoch => 'Storage Epoch';

  @override
  String get storageFailure =>
      'Encrypted local storage could not complete the operation.';

  @override
  String get system => 'System';

  @override
  String get systemDefaultAudioDevice => 'System Default Audio Device';

  @override
  String get systemLanguage => 'Idioma del sistema';

  @override
  String get terminal => 'Terminal';

  @override
  String get today => 'Today';

  @override
  String get todayUpper => 'TODAY';

  @override
  String get transferFailed => 'Transfer failed';

  @override
  String get transfers => 'Transfers';

  @override
  String get transport => 'Transport';

  @override
  String get typeToSearchConversation => 'Type To Search Conversation';

  @override
  String get unavailable => 'Unavailable';

  @override
  String get unblockContact => 'Unblock contact';

  @override
  String get unknown => 'Desconocido';

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
  String get verifiedOnDevice => 'Verified on device';

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
  String get waitingForPeer => 'Waiting for peer';

  @override
  String get waitingToReceive => 'Waiting to receive';

  @override
  String get wakeSources => 'Wake Sources';

  @override
  String get whyAwake => 'Why Awake';

  @override
  String get yesterday => 'Yesterday';

  @override
  String get yourIdentity => 'Your identity';

  @override
  String get yourInvitation => 'Your invitation';

  @override
  String get zeroDelayDeadlines => 'Zero Delay Deadlines';
}

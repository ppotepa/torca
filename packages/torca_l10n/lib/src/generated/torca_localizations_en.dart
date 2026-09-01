// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'torca_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class TorcaLocalizationsEn extends TorcaLocalizations {
  TorcaLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get aboutTorca => 'About Torca';

  @override
  String get accept => 'Accept';

  @override
  String get activeDemands => 'Active demands';

  @override
  String get activeInvitationsDescription =>
      'Your active invitations and pairing requests will appear here.';

  @override
  String get activeLeases => 'Active leases';

  @override
  String get activeTransfers => 'Active';

  @override
  String get allOperations => 'All';

  @override
  String get allowAll => 'Allow all';

  @override
  String get allowDelayedBackgroundDelivery =>
      'Allow delayed background delivery';

  @override
  String get allowDelayedBackgroundDeliveryDescription =>
      'Required before Automatic or Saver can suspend the communication runtime while the app is idle.';

  @override
  String get alwaysAvailable => 'Always available';

  @override
  String get appearance => 'Appearance';

  @override
  String get appearanceTitle => 'Appearance';

  @override
  String get applicationMenu => 'Application menu';

  @override
  String get archiveConversation => 'Archive conversation';

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
  String get audioDeviceUnavailable =>
      'The selected audio device is unavailable.';

  @override
  String get audioOutput => 'Audio output';

  @override
  String get automatic => 'Automatic';

  @override
  String get availabilityMode => 'Availability mode';

  @override
  String get batteryAvailability => 'Battery & availability';

  @override
  String get batteryObservation => 'Battery observation';

  @override
  String get batterySaver => 'Battery saver';

  @override
  String get batterySettingsDescription =>
      'Choose when Torca may defer background work. Incoming work is never silently discarded.';

  @override
  String get batteryTab => 'Battery';

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
  String get blockedSendBlocked =>
      'This contact is blocked. Unblock the contact to send a message.';

  @override
  String get bookmarkMessage => 'Bookmark message';

  @override
  String bootstrapAttempt(String label, int attempt) {
    return '$label · attempt $attempt';
  }

  @override
  String bootstrapProgress(int ready, int total, String elapsed) {
    return '$ready of $total secure checks complete  •  $elapsed';
  }

  @override
  String bootstrapStateDescription(Object code, Object id, Object value) {
    return '$id: $value $code';
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
  String buildTooltip(Object build, Object providerService) {
    return 'Torca build $build\nProvider service: $providerService';
  }

  @override
  String get cancel => 'Cancel';

  @override
  String get cancelInvitation => 'Cancel invitation';

  @override
  String get cancelMessage => 'Cancel message';

  @override
  String get cancelRequest => 'Cancel request';

  @override
  String get cancelled => 'Cancelled';

  @override
  String get chats => 'Chats';

  @override
  String get checkingInvitation => 'Checking invitation...';

  @override
  String get chooseConversation => 'Choose conversation';

  @override
  String get chooseLanguage => 'Choose your language';

  @override
  String get chooseLanguagePolish => 'Choose Language Polish';

  @override
  String get chooseNickname => 'Choose your nickname';

  @override
  String get clearConversationHistory => 'Clear conversation history';

  @override
  String get clearSearch => 'Clear search';

  @override
  String get close => 'Close';

  @override
  String get closeInvitationDescription =>
      'Close this window to continue using the application. The invitation will appear here automatically when the connection is ready.';

  @override
  String get closeScanner => 'Close scanner';

  @override
  String get closeSearch => 'Close search';

  @override
  String get closeToTray => 'Close to tray';

  @override
  String get closeToTrayDescription =>
      'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.';

  @override
  String get closeTooltip => 'Close';

  @override
  String get collapseNavigation => 'Collapse navigation';

  @override
  String get comfortableDensity => 'Comfortable density';

  @override
  String get communicationProvider => 'Communication provider';

  @override
  String get communicationState => 'Communication state';

  @override
  String get compactDensity => 'Compact density';

  @override
  String get completedTransfers => 'Completed';

  @override
  String get connecting => 'Connecting';

  @override
  String connectingPeerThrough(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Connecting to peer through Iroh',
      'memory': 'Connecting to peer through Memory test',
      'other': 'Connecting to peer through $provider',
    });
    return '$_temp0';
  }

  @override
  String get connection => 'Connection';

  @override
  String get connectionDetails => 'Connection details';

  @override
  String get connectionDetailsTitle => 'Connection details';

  @override
  String connectionEvidenceNote(String provider) {
    return 'Quality describes the authenticated direct peer link over $provider. It is runtime evidence, not radio or internet signal strength.';
  }

  @override
  String connectionQuality(Object quality, Object rtt) {
    return 'Connection quality $quality$rtt';
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
  String get contactConnected => 'Contact connected';

  @override
  String get contactConnectedDescription =>
      'The invitation was accepted and this contact is ready to chat.';

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
  String get continueLabel => 'Continue';

  @override
  String get contract => 'Contract';

  @override
  String get contractDecodeFailed =>
      'The client and native runtime use incompatible data. Rebuild and redeploy both.';

  @override
  String get contractSnapshotReadable => 'Contract snapshot readable';

  @override
  String get conversationActions => 'Conversation actions';

  @override
  String get copy => 'Copy';

  @override
  String get copyCode => 'Copy invitation';

  @override
  String get copyFingerprint => 'Copy fingerprint';

  @override
  String get couldNotBlockContact => 'Could not block contact';

  @override
  String get couldNotForwardMessage => 'Could not forward message';

  @override
  String get couldNotQueueAttachment => 'Could not queue attachment';

  @override
  String get couldNotRemoveContact => 'Could not remove contact';

  @override
  String get couldNotRenameContact => 'Could not rename contact';

  @override
  String get couldNotSaveNickname => 'Could not save nickname';

  @override
  String couldNotStartConversation(Object name) {
    return 'Could not start conversation with $name.';
  }

  @override
  String get couldNotStartRadio => 'Could not start transmission';

  @override
  String get couldNotUnblockContact => 'Could not unblock contact';

  @override
  String get couldNotUpdateRadio => 'Could not update Radio mode';

  @override
  String get couldNotUpdateReaction => 'Could not send reaction';

  @override
  String get country => 'Where are you from?';

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
    return '$name (default)';
  }

  @override
  String get deleteMessage => 'Delete for everyone';

  @override
  String get deleteMessageTitle => 'Delete message?';

  @override
  String get delivered => 'Delivered';

  @override
  String get deliveryFailed => 'Delivery failed';

  @override
  String get desktop => 'Desktop';

  @override
  String deviceFingerprint(Object fingerprint) {
    return 'Device fingerprint\n$fingerprint';
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
  String directProviderContact(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Direct Iroh contact',
      'memory': 'Direct Memory test contact',
      'other': 'Direct $provider contact',
    });
    return '$_temp0';
  }

  @override
  String get displayName => 'Display name';

  @override
  String get documentTransfers => 'Documents';

  @override
  String get done => 'Done';

  @override
  String get draft => 'Draft';

  @override
  String get editMessage => 'Edit message';

  @override
  String get emoji => 'Emoji';

  @override
  String get enableNotifications => 'Enable notifications';

  @override
  String get encrypting => 'Encrypting';

  @override
  String get endpoint => 'Endpoint';

  @override
  String get englishCountry => 'England';

  @override
  String get enterSixCharacterCode =>
      'Enter a six-character code or scan the QR code.';

  @override
  String get excellent => 'Excellent';

  @override
  String get expandNavigation => 'Expand navigation';

  @override
  String get exportDiagnostics => 'Export diagnostics';

  @override
  String get exportFailed => 'Export failed';

  @override
  String get exportTorcaDiagnostics => 'Export Torca diagnostics';

  @override
  String get fair => 'Fair';

  @override
  String get fileTransfers => 'Files';

  @override
  String get finalizingContact => 'Finalizing secure contact…';

  @override
  String get fingerprint => 'Fingerprint';

  @override
  String get fingerprintCopied => 'Fingerprint copied';

  @override
  String get focusedOnly => 'Animate focused views';

  @override
  String get followSystem => 'Follow system setting';

  @override
  String get forwardMessage => 'Forward message';

  @override
  String forwardNoAvailableAttachments(Object count) {
    return '$count';
  }

  @override
  String forwardSkippedAttachments(Object count) {
    return '$count';
  }

  @override
  String get fullAnimation => 'Full animation';

  @override
  String get generateInvitation => 'Generate Invitation';

  @override
  String get generatingInvitation => 'Generating…';

  @override
  String get good => 'Good';

  @override
  String get holdToRecordVoiceClip => 'Hold to record a voice clip';

  @override
  String get identicalDeadlineReplacements => 'Identical deadline replacements';

  @override
  String get identity => 'Identity';

  @override
  String get identityChanged =>
      'The contact identity changed. Verify the Safety Number.';

  @override
  String get identityChangedSendBlocked =>
      'Sending is paused until this contact is verified again.';

  @override
  String get incidentDescription =>
      'Run a self-test, mark the current state and export the redacted snapshot. Message text, attachments, audio and secrets are not included.';

  @override
  String get incidentSnapshotSaved =>
      'Incident snapshot saved to this run\'s local diagnostics.';

  @override
  String get incidentTab => 'Incident';

  @override
  String get incidentTools => 'Incident tools';

  @override
  String get incomingMessage => 'Incoming message';

  @override
  String get incompatibleStorageEpoch =>
      'The encrypted local profile is incompatible. Reset local Torca data explicitly before continuing.';

  @override
  String get instantMode => 'Instant mode';

  @override
  String get instantModeEnabled => 'Instant mode enabled';

  @override
  String get invalidInput => 'The supplied value is not valid.';

  @override
  String get invitationCode => 'Invitation code';

  @override
  String get invitationCodeCopied => 'Full invitation copied';

  @override
  String invitationCodeLabel(Object code) {
    return 'Code $code';
  }

  @override
  String invitationExpiresIn(Object countdown) {
    return 'Expires in $countdown';
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
  String get joinRequestWaiting =>
      'Your request is waiting for the invitation owner to verify and accept it.';

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
  String get languageTitle => 'Language';

  @override
  String get lastSeen => 'Last seen';

  @override
  String lastSeenAt(Object time) {
    return 'Last seen $time';
  }

  @override
  String get lastSuccessfulProbe => 'Last successful probe';

  @override
  String get leaseReasons => 'Lease reasons';

  @override
  String get light => 'Light';

  @override
  String get loadCurrentRunLogs => 'Load current run logs';

  @override
  String get loaded => 'Loaded';

  @override
  String get localIdentity => 'Local identity';

  @override
  String get localIdentityCheck => 'Local identity';

  @override
  String get localIdentityNotReady => 'Local identity is not ready';

  @override
  String get localName => 'Local name';

  @override
  String get logsTab => 'Logs';

  @override
  String get markConversationRead => 'Mark as read';

  @override
  String get markIncident => 'Mark incident';

  @override
  String get mediaTransfers => 'Media';

  @override
  String get message => 'Message';

  @override
  String get messageActions => 'Message actions';

  @override
  String get messageCancelled => 'Message cancelled';

  @override
  String get messageCopied => 'Message copied';

  @override
  String get messageDeleted => 'Message deleted';

  @override
  String get messageDetails => 'Message details';

  @override
  String get messageEdited => 'Message edited';

  @override
  String get messageForwarded => 'Message forwarded';

  @override
  String get messageQueued => 'Queued — waiting for a direct peer connection';

  @override
  String get messageSenderContact => 'Contact';

  @override
  String get messageSenderYou => 'You';

  @override
  String messageTooLong(int maximum) {
    return 'Messages can contain at most $maximum characters.';
  }

  @override
  String get meteredTransfers => 'Metered network transfers';

  @override
  String get microphone => 'Microphone';

  @override
  String get microphonePermissionRequired =>
      'Microphone access is required to transmit.';

  @override
  String get modern => 'Modern';

  @override
  String get muteConversation => 'Mute conversation';

  @override
  String get nativeBridge => 'Native bridge';

  @override
  String get nativeLogTails => 'Native log tails';

  @override
  String get nativeLogTailsDescription =>
      'Loads a bounded, redacted tail from current-run native logs only. This explicit read does not keep a watcher alive.';

  @override
  String get networkUnavailable =>
      'The selected communication connection is currently unavailable.';

  @override
  String get never => 'Never';

  @override
  String get newContact => 'New contact';

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
  String get nextDeadline => 'Next deadline';

  @override
  String get nickname => 'Nickname';

  @override
  String get nicknameIntro =>
      'The selected communication provider is ready. This name will be shown to contacts.';

  @override
  String get nicknameRequired => 'Nickname is required';

  @override
  String get noActiveTransfers => 'No active transfers.';

  @override
  String get noChatsMatch => 'No chats match your search';

  @override
  String get noContactsPaired => 'No contacts paired';

  @override
  String get noContactsYet => 'No contacts yet';

  @override
  String get noForwardableContent =>
      'This message has no content that can be forwarded.';

  @override
  String get noInvitations => 'No invitations';

  @override
  String get noMatchingMessages => 'No matching messages.';

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
  String get notificationsTitle => 'Notifications';

  @override
  String get observationRecording => 'recording';

  @override
  String get observationRecordingDescription =>
      'Recording deltas since the observation baseline.';

  @override
  String get observationState => 'State';

  @override
  String get observationStopped => 'stopped';

  @override
  String get observationStoppedDescription =>
      'Start before an idle or recovery scenario to record only new work.';

  @override
  String get observationWork => 'Work';

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
  String get originalMessageUnavailable => 'Original message unavailable';

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
  String get pairingCompletedMessage =>
      'The contact was added securely. Open the private conversation now.';

  @override
  String get pairingExpired => 'The pairing invitation has expired.';

  @override
  String get pairingInactiveMessage =>
      'This invitation is no longer active. The other device will receive the same final state.';

  @override
  String get pairingProviderMismatch =>
      'This invitation belongs to a different communication provider.';

  @override
  String get pairingQrSemanticLabel => 'Torca pairing invitation QR code';

  @override
  String get pairingRequestDescription =>
      'This device joined your invitation. Review the contact details before accepting.';

  @override
  String pairingStateLabel(String state) {
    String _temp0 = intl.Intl.selectLogic(state, {
      'open': 'Open',
      'peer_joined': 'Peer joined',
      'awaiting_approval': 'Awaiting approval',
      'approved': 'Approved',
      'completed': 'Completed',
      'rejected': 'Rejected',
      'cancelled': 'Cancelled',
      'expired': 'Expired',
      'unknown': 'Unknown',
      'other': 'Unknown',
    });
    return '$_temp0';
  }

  @override
  String get pauseAll => 'Pause all transfers';

  @override
  String get pauseLarge => 'Pause large files';

  @override
  String get peerOffline => 'Peer is offline';

  @override
  String get peerState => 'P2P state';

  @override
  String get pendingOperations => 'Pending';

  @override
  String get pinConversation => 'Pin conversation';

  @override
  String get playVoiceMessage => 'Play voice message';

  @override
  String get polishCountry => 'Poland';

  @override
  String get poor => 'Poor';

  @override
  String get preparingDownload => 'Preparing download';

  @override
  String get preparingPrivateSpace => 'Preparing your private space';

  @override
  String get preparingPrivateSpaceDescription =>
      'Setting up encrypted storage and secure communication. You can safely leave this screen open.';

  @override
  String get preparingSecureCopy => 'Preparing secure copy';

  @override
  String get preparingUpload => 'Preparing upload';

  @override
  String get presence => 'Presence';

  @override
  String get privacy => 'Privacy';

  @override
  String get privacyTitle => 'Privacy';

  @override
  String get productVersion => 'Product version';

  @override
  String get profileNotReady => 'The secure profile is not ready yet.';

  @override
  String get providerEndpoint => 'Provider endpoint';

  @override
  String get providerEndpointAvailable => 'Available';

  @override
  String get providerEndpointUnavailable => 'Unavailable';

  @override
  String providerName(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Iroh',
      'memory': 'Memory test',
      'other': '$provider',
    });
    return '$_temp0';
  }

  @override
  String providerReady(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Iroh ready',
      'memory': 'Memory test ready',
      'other': '$provider ready',
    });
    return '$_temp0';
  }

  @override
  String providerReconnecting(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Iroh reconnecting',
      'memory': 'Memory test reconnecting',
      'other': '$provider reconnecting',
    });
    return '$_temp0';
  }

  @override
  String providerStarting(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Iroh starting',
      'memory': 'Memory test starting',
      'other': '$provider starting',
    });
    return '$_temp0';
  }

  @override
  String providerStateLabel(String provider, String state) {
    return '$provider: $state';
  }

  @override
  String get published => 'Published';

  @override
  String get quality => 'Quality';

  @override
  String get queued => 'Queued';

  @override
  String get radioChannelInterrupted => 'Radio channel was interrupted';

  @override
  String get radioChannelReady => 'Private Radio channel is ready';

  @override
  String get radioChannelRestored => 'Radio channel was restored';

  @override
  String get radioConnecting => 'Connecting the private audio channel...';

  @override
  String radioDisabledBy(Object actor) {
    return '$actor disabled Radio mode';
  }

  @override
  String radioEnabledBy(Object actor) {
    return '$actor enabled Radio mode';
  }

  @override
  String get radioMode => 'Radio mode';

  @override
  String get radioModeDescription =>
      'Short push-to-talk transmissions of up to 10 seconds. Radio becomes available only after both contacts enable it.';

  @override
  String get radioReady => 'Hold to talk';

  @override
  String radioReceiving(Object name) {
    return '$name is transmitting';
  }

  @override
  String get radioReconnecting => 'Radio is reconnecting...';

  @override
  String get radioRequestingFloor => 'Requesting the channel...';

  @override
  String get radioTransmitting => 'Transmitting';

  @override
  String radioTransportFailure(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'endpoint_unavailable': 'endpoint unavailable',
      'connect_timeout': 'connection timeout',
      'stream_reset': 'stream reset',
      'idle_timeout': 'idle timeout',
      'network_changed': 'network changed',
      'worker_unavailable': 'audio worker unavailable',
      'protocol': 'protocol error',
      'other': 'unknown transport error',
    });
    return 'Radio: $_temp0';
  }

  @override
  String get radioUnavailable => 'Radio is temporarily unavailable';

  @override
  String get radioWaitingForPeer => 'Waiting for the contact to enable Radio';

  @override
  String get rawDiagnostics => 'Raw diagnostics';

  @override
  String get reactToMessage => 'React';

  @override
  String get read => 'Read';

  @override
  String get receivingSecurely => 'Receiving securely';

  @override
  String get recentEmoji => 'Recently used';

  @override
  String get recentInvitations => 'Recent invitations';

  @override
  String get reconnectAttempts => 'Reconnect attempts';

  @override
  String get reconnecting => 'Reconnecting';

  @override
  String reconnectingPeerThrough(String provider) {
    String _temp0 = intl.Intl.selectLogic(provider, {
      'iroh': 'Reconnecting to peer through Iroh',
      'memory': 'Reconnecting to peer through Memory test',
      'other': 'Reconnecting to peer through $provider',
    });
    return '$_temp0';
  }

  @override
  String get reconnectingShort => 'Reconnecting';

  @override
  String get recordingTransfers => 'Recordings';

  @override
  String get redactedDeveloperEventStream => 'Redacted developer event stream';

  @override
  String get redactedHealthEventsReadable => 'Redacted health events readable';

  @override
  String get redactedSchedulerDescription =>
      'Redacted scheduler explanation; contact identifiers are never shown here.';

  @override
  String get reduceMotion => 'Reduce motion';

  @override
  String get refresh => 'Refresh';

  @override
  String get refreshProviderRoute => 'Refresh provider route';

  @override
  String get regressionScore => 'Regression score';

  @override
  String get reject => 'Reject';

  @override
  String remoteIdentity(String id) {
    return 'Identity $id';
  }

  @override
  String get remoteIdentityTitle => 'Remote identity';

  @override
  String get remove => 'Remove';

  @override
  String get removeAttachment => 'Remove attachment';

  @override
  String get removeBookmark => 'Remove bookmark';

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
  String get resetBaseline => 'Reset baseline';

  @override
  String get resetVerification => 'Reset verification';

  @override
  String get restartApplication => 'Restart application';

  @override
  String get restoreConversation => 'Restore conversation';

  @override
  String get retry => 'Retry';

  @override
  String get retryGeneration => 'Retry generation';

  @override
  String get retryNow => 'Retry now';

  @override
  String get retrying => 'Retrying…';

  @override
  String get roundTrip => 'Round trip';

  @override
  String get route => 'Provider route';

  @override
  String get routeRefreshRequested => 'Provider route refresh requested.';

  @override
  String get routeRefreshRequired =>
      'The communication route is being refreshed. Try again shortly.';

  @override
  String get runSelfTest => 'Run self-test';

  @override
  String get runtimeHealth => 'Runtime health';

  @override
  String runtimeNotReadyDiagnostic(Object provider) {
    return '$provider';
  }

  @override
  String get runtimePreparationFailed =>
      'Torca could not prepare the local encrypted runtime. Your identity has not been changed.';

  @override
  String get runtimeTab => 'Runtime';

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
  String get save => 'Save';

  @override
  String get saveAs => 'Save as';

  @override
  String get saveAttachment => 'Save attachment';

  @override
  String get saving => 'Saving…';

  @override
  String get scanQr => 'Scan QR';

  @override
  String get scheduledWork => 'Scheduled work';

  @override
  String get searchChats => 'Search chats';

  @override
  String get searchConversationHint => 'Search this conversation';

  @override
  String get searchMessages => 'Search messages';

  @override
  String searchResultsCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count results',
      one: '$count result',
    );
    return '$_temp0';
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
  String sentAt(Object time) {
    return 'Sent $time';
  }

  @override
  String deliveredAt(Object time) {
    return 'Delivered $time';
  }

  @override
  String seenAt(Object time) {
    return 'Seen at $time';
  }

  @override
  String receivedAt(Object time) {
    return 'Received $time';
  }

  @override
  String get settings => 'Settings';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get sharedMedia => 'Shared media and files';

  @override
  String sharedMediaCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items',
      one: '1 item',
    );
    return '$_temp0';
  }

  @override
  String get sourceCommit => 'Source commit';

  @override
  String get startConversation => 'Start conversation';

  @override
  String get startObservation => 'Start observation';

  @override
  String get startingSecureNetwork => 'Starting communication…';

  @override
  String get startingShort => 'Starting';

  @override
  String get state => 'State';

  @override
  String get staticIdle => 'Static when idle';

  @override
  String get status => 'Status';

  @override
  String get stopObservation => 'Stop observation';

  @override
  String get storageEpoch => 'Storage epoch';

  @override
  String get storageFailure =>
      'Encrypted local storage could not complete the operation.';

  @override
  String get system => 'System';

  @override
  String get systemDefaultAudioDevice => 'System default device';

  @override
  String get systemLanguage => 'System language';

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
  String get typeToSearchConversation => 'Type to search this conversation.';

  @override
  String get unavailable => 'Unavailable';

  @override
  String get unblockContact => 'Unblock contact';

  @override
  String get unknown => 'Unknown';

  @override
  String get unknownCountry => 'Unknown';

  @override
  String get unmuteConversation => 'Unmute conversation';

  @override
  String get unpinConversation => 'Unpin conversation';

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
  String get verifyContact => 'Verify contact';

  @override
  String get verifyFingerprintBeforeAccepting =>
      'A device joined this invitation. Verify the fingerprint before accepting the contact.';

  @override
  String get visualActivity => 'Avatar and visual activity';

  @override
  String voiceClipRecording(Object secondsLeft) {
    return 'Recording voice clip, $secondsLeft s remaining';
  }

  @override
  String get voiceClipRecordingFailed => 'Could not record the voice clip.';

  @override
  String get voiceMessage => 'Voice message';

  @override
  String get voiceMessagePlayed => 'Played';

  @override
  String get voiceMessageReady => 'Ready to play';

  @override
  String waitingForDependency(Object dependency) {
    return 'Waiting for: $dependency';
  }

  @override
  String get waitingForPeer => 'Waiting for peer';

  @override
  String get waitingToReceive => 'Waiting to receive';

  @override
  String get wakeSources => 'Wake sources';

  @override
  String get whyAwake => 'Why awake';

  @override
  String get yesterday => 'Yesterday';

  @override
  String get yourIdentity => 'Your identity';

  @override
  String get yourInvitation => 'Your invitation';

  @override
  String get zeroDelayDeadlines => 'Zero-delay deadlines';
}

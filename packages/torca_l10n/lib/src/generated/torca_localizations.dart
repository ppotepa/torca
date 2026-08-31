import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'torca_localizations_de.dart';
import 'torca_localizations_en.dart';
import 'torca_localizations_es.dart';
import 'torca_localizations_fr.dart';
import 'torca_localizations_pl.dart';
import 'torca_localizations_uk.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of TorcaLocalizations
/// returned by `TorcaLocalizations.of(context)`.
///
/// Applications need to include `TorcaLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/torca_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: TorcaLocalizations.localizationsDelegates,
///   supportedLocales: TorcaLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the TorcaLocalizations.supportedLocales
/// property.
abstract class TorcaLocalizations {
  TorcaLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static TorcaLocalizations? of(BuildContext context) {
    return Localizations.of<TorcaLocalizations>(context, TorcaLocalizations);
  }

  static const LocalizationsDelegate<TorcaLocalizations> delegate =
      _TorcaLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('pl'),
    Locale('de'),
    Locale('es'),
    Locale('fr'),
    Locale('uk'),
  ];

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @appearance.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearance;

  /// No description provided for @system.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get system;

  /// No description provided for @light.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get light;

  /// No description provided for @dark.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get dark;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// No description provided for @languageSystem.
  ///
  /// In en, this message translates to:
  /// **'System language'**
  String get languageSystem;

  /// No description provided for @languageEnglish.
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get languageEnglish;

  /// No description provided for @languagePolish.
  ///
  /// In en, this message translates to:
  /// **'Polish'**
  String get languagePolish;

  /// No description provided for @privacy.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get privacy;

  /// No description provided for @sendReadReceipts.
  ///
  /// In en, this message translates to:
  /// **'Send read receipts'**
  String get sendReadReceipts;

  /// No description provided for @sendReadReceiptsDescription.
  ///
  /// In en, this message translates to:
  /// **'Messages are marked read locally, but contacts see the Read state only when this option is enabled.'**
  String get sendReadReceiptsDescription;

  /// No description provided for @notifications.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notifications;

  /// No description provided for @enableNotifications.
  ///
  /// In en, this message translates to:
  /// **'Enable notifications'**
  String get enableNotifications;

  /// No description provided for @notificationPrivacy.
  ///
  /// In en, this message translates to:
  /// **'Show private-message notifications without message content.'**
  String get notificationPrivacy;

  /// No description provided for @desktop.
  ///
  /// In en, this message translates to:
  /// **'Desktop'**
  String get desktop;

  /// No description provided for @closeToTray.
  ///
  /// In en, this message translates to:
  /// **'Close to tray'**
  String get closeToTray;

  /// No description provided for @closeToTrayDescription.
  ///
  /// In en, this message translates to:
  /// **'Keep Torca running when the main window is closed. Disable this to quit Torca when closing the window.'**
  String get closeToTrayDescription;

  /// No description provided for @pairContact.
  ///
  /// In en, this message translates to:
  /// **'Pair contact'**
  String get pairContact;

  /// No description provided for @newPrivateMessage.
  ///
  /// In en, this message translates to:
  /// **'New private message'**
  String get newPrivateMessage;

  /// No description provided for @message.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get message;

  /// No description provided for @senderYou.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get senderYou;

  /// No description provided for @senderContact.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get senderContact;

  /// No description provided for @outgoingMessage.
  ///
  /// In en, this message translates to:
  /// **'Outgoing message'**
  String get outgoingMessage;

  /// No description provided for @incomingMessage.
  ///
  /// In en, this message translates to:
  /// **'Incoming message'**
  String get incomingMessage;

  /// No description provided for @sent.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get sent;

  /// No description provided for @delivered.
  ///
  /// In en, this message translates to:
  /// **'Delivered'**
  String get delivered;

  /// No description provided for @read.
  ///
  /// In en, this message translates to:
  /// **'Read'**
  String get read;

  /// No description provided for @messageQueued.
  ///
  /// In en, this message translates to:
  /// **'Queued — waiting for a direct peer connection'**
  String get messageQueued;

  /// No description provided for @deliveryFailed.
  ///
  /// In en, this message translates to:
  /// **'Delivery failed'**
  String get deliveryFailed;

  /// No description provided for @reply.
  ///
  /// In en, this message translates to:
  /// **'Reply'**
  String get reply;

  /// No description provided for @sendMessage.
  ///
  /// In en, this message translates to:
  /// **'Send message'**
  String get sendMessage;

  /// No description provided for @attachFiles.
  ///
  /// In en, this message translates to:
  /// **'Attach files'**
  String get attachFiles;

  /// No description provided for @newMessages.
  ///
  /// In en, this message translates to:
  /// **'New messages'**
  String get newMessages;

  /// No description provided for @jumpToLatest.
  ///
  /// In en, this message translates to:
  /// **'Jump to latest message'**
  String get jumpToLatest;

  /// No description provided for @today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get today;

  /// No description provided for @yesterday.
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get yesterday;

  /// No description provided for @retryNow.
  ///
  /// In en, this message translates to:
  /// **'Retry now'**
  String get retryNow;

  /// No description provided for @retrying.
  ///
  /// In en, this message translates to:
  /// **'Retrying…'**
  String get retrying;

  /// No description provided for @blocked.
  ///
  /// In en, this message translates to:
  /// **'Blocked'**
  String get blocked;

  /// No description provided for @startConversation.
  ///
  /// In en, this message translates to:
  /// **'Start conversation'**
  String get startConversation;

  /// No description provided for @connection.
  ///
  /// In en, this message translates to:
  /// **'Connection'**
  String get connection;

  /// No description provided for @state.
  ///
  /// In en, this message translates to:
  /// **'State'**
  String get state;

  /// No description provided for @quality.
  ///
  /// In en, this message translates to:
  /// **'Quality'**
  String get quality;

  /// No description provided for @connectionDetails.
  ///
  /// In en, this message translates to:
  /// **'Connection details'**
  String get connectionDetails;

  /// No description provided for @contactActions.
  ///
  /// In en, this message translates to:
  /// **'Contact actions'**
  String get contactActions;

  /// No description provided for @renameContact.
  ///
  /// In en, this message translates to:
  /// **'Rename contact'**
  String get renameContact;

  /// No description provided for @unblockContact.
  ///
  /// In en, this message translates to:
  /// **'Unblock contact'**
  String get unblockContact;

  /// No description provided for @blockContact.
  ///
  /// In en, this message translates to:
  /// **'Block contact'**
  String get blockContact;

  /// No description provided for @removeContact.
  ///
  /// In en, this message translates to:
  /// **'Remove contact'**
  String get removeContact;

  /// No description provided for @localName.
  ///
  /// In en, this message translates to:
  /// **'Local name'**
  String get localName;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// No description provided for @blockContactDescription.
  ///
  /// In en, this message translates to:
  /// **'Torca will close the peer connection and will not reconnect until you unblock this contact.'**
  String get blockContactDescription;

  /// No description provided for @removeContactDescription.
  ///
  /// In en, this message translates to:
  /// **'This removes the local relationship, conversation history, pending work and protected peer credential.'**
  String get removeContactDescription;

  /// No description provided for @couldNotRenameContact.
  ///
  /// In en, this message translates to:
  /// **'Could not rename contact'**
  String get couldNotRenameContact;

  /// No description provided for @couldNotBlockContact.
  ///
  /// In en, this message translates to:
  /// **'Could not block contact'**
  String get couldNotBlockContact;

  /// No description provided for @couldNotUnblockContact.
  ///
  /// In en, this message translates to:
  /// **'Could not unblock contact'**
  String get couldNotUnblockContact;

  /// No description provided for @couldNotRemoveContact.
  ///
  /// In en, this message translates to:
  /// **'Could not remove contact'**
  String get couldNotRemoveContact;

  /// No description provided for @profileNotReady.
  ///
  /// In en, this message translates to:
  /// **'The secure profile is not ready yet.'**
  String get profileNotReady;

  /// No description provided for @identityChanged.
  ///
  /// In en, this message translates to:
  /// **'The contact identity changed. Verify the Safety Number.'**
  String get identityChanged;

  /// No description provided for @pairingExpired.
  ///
  /// In en, this message translates to:
  /// **'The pairing invitation has expired.'**
  String get pairingExpired;

  /// No description provided for @itemAlreadyExists.
  ///
  /// In en, this message translates to:
  /// **'This item already exists.'**
  String get itemAlreadyExists;

  /// No description provided for @itemNotFound.
  ///
  /// In en, this message translates to:
  /// **'The item is no longer available.'**
  String get itemNotFound;

  /// No description provided for @invalidInput.
  ///
  /// In en, this message translates to:
  /// **'The supplied value is not valid.'**
  String get invalidInput;

  /// No description provided for @storageFailure.
  ///
  /// In en, this message translates to:
  /// **'Encrypted local storage could not complete the operation.'**
  String get storageFailure;

  /// No description provided for @networkUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The selected communication connection is currently unavailable.'**
  String get networkUnavailable;

  /// No description provided for @runtimeUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The secure Torca runtime is currently unavailable.'**
  String get runtimeUnavailable;

  /// No description provided for @contractDecodeFailed.
  ///
  /// In en, this message translates to:
  /// **'The client and native runtime use incompatible data. Rebuild and redeploy both.'**
  String get contractDecodeFailed;

  /// No description provided for @operationFailed.
  ///
  /// In en, this message translates to:
  /// **'The operation could not be completed.'**
  String get operationFailed;

  /// No description provided for @yourIdentity.
  ///
  /// In en, this message translates to:
  /// **'Your identity'**
  String get yourIdentity;

  /// No description provided for @localIdentity.
  ///
  /// In en, this message translates to:
  /// **'Local identity'**
  String get localIdentity;

  /// No description provided for @displayName.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get displayName;

  /// No description provided for @unavailable.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get unavailable;

  /// No description provided for @applicationMenu.
  ///
  /// In en, this message translates to:
  /// **'Application menu'**
  String get applicationMenu;

  /// No description provided for @newPairing.
  ///
  /// In en, this message translates to:
  /// **'New pairing'**
  String get newPairing;

  /// No description provided for @newPairingRequest.
  ///
  /// In en, this message translates to:
  /// **'New pairing request'**
  String get newPairingRequest;

  /// No description provided for @newDevice.
  ///
  /// In en, this message translates to:
  /// **'New device'**
  String get newDevice;

  /// No description provided for @pairingRequestDescription.
  ///
  /// In en, this message translates to:
  /// **'This device joined your invitation. Review the contact details before accepting.'**
  String get pairingRequestDescription;

  /// No description provided for @diagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get diagnostics;

  /// No description provided for @aboutTorca.
  ///
  /// In en, this message translates to:
  /// **'About Torca'**
  String get aboutTorca;

  /// No description provided for @connectionDetailsTitle.
  ///
  /// In en, this message translates to:
  /// **'Connection details'**
  String get connectionDetailsTitle;

  /// No description provided for @contactUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This contact is no longer available.'**
  String get contactUnavailable;

  /// No description provided for @status.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get status;

  /// No description provided for @transport.
  ///
  /// In en, this message translates to:
  /// **'Transport'**
  String get transport;

  /// No description provided for @roundTrip.
  ///
  /// In en, this message translates to:
  /// **'Round trip'**
  String get roundTrip;

  /// No description provided for @lastSuccessfulProbe.
  ///
  /// In en, this message translates to:
  /// **'Last successful probe'**
  String get lastSuccessfulProbe;

  /// No description provided for @consecutiveFailures.
  ///
  /// In en, this message translates to:
  /// **'Consecutive failures'**
  String get consecutiveFailures;

  /// No description provided for @reconnectAttempts.
  ///
  /// In en, this message translates to:
  /// **'Reconnect attempts'**
  String get reconnectAttempts;

  /// No description provided for @open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// No description provided for @saveAs.
  ///
  /// In en, this message translates to:
  /// **'Save as'**
  String get saveAs;

  /// No description provided for @messageDetails.
  ///
  /// In en, this message translates to:
  /// **'Message details'**
  String get messageDetails;

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @messageCopied.
  ///
  /// In en, this message translates to:
  /// **'Message copied'**
  String get messageCopied;

  /// No description provided for @attachmentSaved.
  ///
  /// In en, this message translates to:
  /// **'Attachment saved'**
  String get attachmentSaved;

  /// No description provided for @diagnosticsExported.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics exported'**
  String get diagnosticsExported;

  /// No description provided for @exportFailed.
  ///
  /// In en, this message translates to:
  /// **'Export failed'**
  String get exportFailed;

  /// No description provided for @connectionSelfTest.
  ///
  /// In en, this message translates to:
  /// **'Connection self-test'**
  String get connectionSelfTest;

  /// No description provided for @runSelfTest.
  ///
  /// In en, this message translates to:
  /// **'Run self-test'**
  String get runSelfTest;

  /// No description provided for @exportDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Export diagnostics'**
  String get exportDiagnostics;

  /// No description provided for @noMessagesYet.
  ///
  /// In en, this message translates to:
  /// **'No messages yet'**
  String get noMessagesYet;

  /// No description provided for @contactLabel.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get contactLabel;

  /// No description provided for @closeTooltip.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get closeTooltip;

  /// No description provided for @secureRuntimeNotReady.
  ///
  /// In en, this message translates to:
  /// **'Secure runtime is not ready'**
  String get secureRuntimeNotReady;

  /// No description provided for @runtimePreparationFailed.
  ///
  /// In en, this message translates to:
  /// **'Torca could not prepare the local encrypted runtime. Your identity has not been changed.'**
  String get runtimePreparationFailed;

  /// No description provided for @modern.
  ///
  /// In en, this message translates to:
  /// **'Modern'**
  String get modern;

  /// No description provided for @terminal.
  ///
  /// In en, this message translates to:
  /// **'Terminal'**
  String get terminal;

  /// No description provided for @compactDensity.
  ///
  /// In en, this message translates to:
  /// **'Compact density'**
  String get compactDensity;

  /// No description provided for @comfortableDensity.
  ///
  /// In en, this message translates to:
  /// **'Comfortable density'**
  String get comfortableDensity;

  /// No description provided for @reduceMotion.
  ///
  /// In en, this message translates to:
  /// **'Reduce motion'**
  String get reduceMotion;

  /// No description provided for @rawDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Raw diagnostics'**
  String get rawDiagnostics;

  /// No description provided for @redactedDeveloperEventStream.
  ///
  /// In en, this message translates to:
  /// **'Redacted developer event stream'**
  String get redactedDeveloperEventStream;

  /// No description provided for @diagnosticsStream.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics stream'**
  String get diagnosticsStream;

  /// No description provided for @excellent.
  ///
  /// In en, this message translates to:
  /// **'Excellent'**
  String get excellent;

  /// No description provided for @good.
  ///
  /// In en, this message translates to:
  /// **'Good'**
  String get good;

  /// No description provided for @fair.
  ///
  /// In en, this message translates to:
  /// **'Fair'**
  String get fair;

  /// No description provided for @poor.
  ///
  /// In en, this message translates to:
  /// **'Poor'**
  String get poor;

  /// No description provided for @unknown.
  ///
  /// In en, this message translates to:
  /// **'Unknown'**
  String get unknown;

  /// No description provided for @closeScanner.
  ///
  /// In en, this message translates to:
  /// **'Close scanner'**
  String get closeScanner;

  /// No description provided for @generatingInvitation.
  ///
  /// In en, this message translates to:
  /// **'Generating…'**
  String get generatingInvitation;

  /// No description provided for @retryGeneration.
  ///
  /// In en, this message translates to:
  /// **'Retry generation'**
  String get retryGeneration;

  /// No description provided for @yourInvitation.
  ///
  /// In en, this message translates to:
  /// **'Your invitation'**
  String get yourInvitation;

  /// No description provided for @joinInvitation.
  ///
  /// In en, this message translates to:
  /// **'Join invitation'**
  String get joinInvitation;

  /// No description provided for @checkingInvitation.
  ///
  /// In en, this message translates to:
  /// **'Checking invitation...'**
  String get checkingInvitation;

  /// No description provided for @invitationCode.
  ///
  /// In en, this message translates to:
  /// **'Code {code}'**
  String invitationCode(Object code);

  /// No description provided for @enterSixCharacterCode.
  ///
  /// In en, this message translates to:
  /// **'Enter a six-character code or scan the QR code.'**
  String get enterSixCharacterCode;

  /// No description provided for @pairingBootstrapRequired.
  ///
  /// In en, this message translates to:
  /// **'For this provider, scan the QR code or paste the full invitation link.'**
  String get pairingBootstrapRequired;

  /// No description provided for @pairingProviderMismatch.
  ///
  /// In en, this message translates to:
  /// **'This invitation belongs to a different communication provider.'**
  String get pairingProviderMismatch;

  /// No description provided for @providerEndpoint.
  ///
  /// In en, this message translates to:
  /// **'Provider endpoint'**
  String get providerEndpoint;

  /// No description provided for @invitationGenerating.
  ///
  /// In en, this message translates to:
  /// **'Generating a private invitation...'**
  String get invitationGenerating;

  /// No description provided for @invitationWaitingForNetwork.
  ///
  /// In en, this message translates to:
  /// **'Invitation is waiting for the network.'**
  String get invitationWaitingForNetwork;

  /// No description provided for @invitationQueued.
  ///
  /// In en, this message translates to:
  /// **'Invitation queued for the secure network.'**
  String get invitationQueued;

  /// No description provided for @invitationOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'Invitation operation failed'**
  String get invitationOperationFailed;

  /// No description provided for @invitationJoinSent.
  ///
  /// In en, this message translates to:
  /// **'Join request sent. You will be notified when it is accepted.'**
  String get invitationJoinSent;

  /// No description provided for @invitationSavedLocally.
  ///
  /// In en, this message translates to:
  /// **'Saved locally. It will retry when the selected communication provider is ready.'**
  String get invitationSavedLocally;

  /// No description provided for @openConversation.
  ///
  /// In en, this message translates to:
  /// **'Open conversation'**
  String get openConversation;

  /// No description provided for @noMessagesYetDescription.
  ///
  /// In en, this message translates to:
  /// **'Messages are sent directly through the selected communication provider.'**
  String get noMessagesYetDescription;

  /// No description provided for @attachmentSyncing.
  ///
  /// In en, this message translates to:
  /// **'Attachment is syncing…'**
  String get attachmentSyncing;

  /// No description provided for @closeSearch.
  ///
  /// In en, this message translates to:
  /// **'Close search'**
  String get closeSearch;

  /// No description provided for @preparingUpload.
  ///
  /// In en, this message translates to:
  /// **'Preparing upload'**
  String get preparingUpload;

  /// No description provided for @preparingDownload.
  ///
  /// In en, this message translates to:
  /// **'Preparing download'**
  String get preparingDownload;

  /// No description provided for @preparingSecureCopy.
  ///
  /// In en, this message translates to:
  /// **'Preparing secure copy'**
  String get preparingSecureCopy;

  /// No description provided for @encrypting.
  ///
  /// In en, this message translates to:
  /// **'Encrypting'**
  String get encrypting;

  /// No description provided for @waitingToReceive.
  ///
  /// In en, this message translates to:
  /// **'Waiting to receive'**
  String get waitingToReceive;

  /// No description provided for @waitingForPeer.
  ///
  /// In en, this message translates to:
  /// **'Waiting for peer'**
  String get waitingForPeer;

  /// No description provided for @sendingSecurely.
  ///
  /// In en, this message translates to:
  /// **'Sending securely'**
  String get sendingSecurely;

  /// No description provided for @receivingSecurely.
  ///
  /// In en, this message translates to:
  /// **'Receiving securely'**
  String get receivingSecurely;

  /// No description provided for @verifiedOnDevice.
  ///
  /// In en, this message translates to:
  /// **'Verified on device'**
  String get verifiedOnDevice;

  /// No description provided for @transferFailed.
  ///
  /// In en, this message translates to:
  /// **'Transfer failed'**
  String get transferFailed;

  /// No description provided for @cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get cancelled;

  /// No description provided for @attachmentAckTimeout.
  ///
  /// In en, this message translates to:
  /// **'waiting for peer acknowledgement'**
  String get attachmentAckTimeout;

  /// No description provided for @attachmentPeerUnavailable.
  ///
  /// In en, this message translates to:
  /// **'peer unavailable'**
  String get attachmentPeerUnavailable;

  /// No description provided for @attachmentIntegrityFailed.
  ///
  /// In en, this message translates to:
  /// **'integrity check failed'**
  String get attachmentIntegrityFailed;

  /// No description provided for @attachmentStorageFailed.
  ///
  /// In en, this message translates to:
  /// **'local storage failed'**
  String get attachmentStorageFailed;

  /// No description provided for @attachmentMessagePending.
  ///
  /// In en, this message translates to:
  /// **'waiting for message'**
  String get attachmentMessagePending;

  /// No description provided for @attachmentDependencyMissing.
  ///
  /// In en, this message translates to:
  /// **'waiting for conversation'**
  String get attachmentDependencyMissing;

  /// No description provided for @attachmentRetryAvailable.
  ///
  /// In en, this message translates to:
  /// **'retry available'**
  String get attachmentRetryAvailable;

  /// No description provided for @attachmentOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'Attachment operation failed'**
  String get attachmentOperationFailed;

  /// No description provided for @couldNotQueueAttachment.
  ///
  /// In en, this message translates to:
  /// **'Could not queue attachment'**
  String get couldNotQueueAttachment;

  /// No description provided for @saveAttachment.
  ///
  /// In en, this message translates to:
  /// **'Save attachment'**
  String get saveAttachment;

  /// No description provided for @buildAndConnectionInfo.
  ///
  /// In en, this message translates to:
  /// **'Build & connection info'**
  String get buildAndConnectionInfo;

  /// No description provided for @pairContactHint.
  ///
  /// In en, this message translates to:
  /// **'Pair a contact to start a conversation.'**
  String get pairContactHint;

  /// No description provided for @contacts.
  ///
  /// In en, this message translates to:
  /// **'Contacts'**
  String get contacts;

  /// No description provided for @invitations.
  ///
  /// In en, this message translates to:
  /// **'Invitations'**
  String get invitations;

  /// No description provided for @selectConversation.
  ///
  /// In en, this message translates to:
  /// **'Select a conversation'**
  String get selectConversation;

  /// No description provided for @createManageInvitations.
  ///
  /// In en, this message translates to:
  /// **'Create and manage short-lived private contact invitations.'**
  String get createManageInvitations;

  /// No description provided for @generateInvitation.
  ///
  /// In en, this message translates to:
  /// **'Generate Invitation'**
  String get generateInvitation;

  /// No description provided for @copyCode.
  ///
  /// In en, this message translates to:
  /// **'Copy invitation'**
  String get copyCode;

  /// No description provided for @invitationCodeCopied.
  ///
  /// In en, this message translates to:
  /// **'Full invitation copied'**
  String get invitationCodeCopied;

  /// No description provided for @done.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @accept.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get accept;

  /// No description provided for @reject.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get reject;

  /// No description provided for @cancelRequest.
  ///
  /// In en, this message translates to:
  /// **'Cancel request'**
  String get cancelRequest;

  /// No description provided for @cancelInvitation.
  ///
  /// In en, this message translates to:
  /// **'Cancel invitation'**
  String get cancelInvitation;

  /// No description provided for @copy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copy;

  /// No description provided for @noContactsYet.
  ///
  /// In en, this message translates to:
  /// **'No contacts yet'**
  String get noContactsYet;

  /// No description provided for @createInvitationForContact.
  ///
  /// In en, this message translates to:
  /// **'Create an invitation to add a private contact.'**
  String get createInvitationForContact;

  /// No description provided for @openChat.
  ///
  /// In en, this message translates to:
  /// **'Open chat'**
  String get openChat;

  /// No description provided for @contactInformation.
  ///
  /// In en, this message translates to:
  /// **'Contact information'**
  String get contactInformation;

  /// No description provided for @noInvitations.
  ///
  /// In en, this message translates to:
  /// **'No invitations'**
  String get noInvitations;

  /// No description provided for @activeInvitationsDescription.
  ///
  /// In en, this message translates to:
  /// **'Your active invitations and pairing requests will appear here.'**
  String get activeInvitationsDescription;

  /// No description provided for @recentInvitations.
  ///
  /// In en, this message translates to:
  /// **'Recent invitations'**
  String get recentInvitations;

  /// No description provided for @createdInvitation.
  ///
  /// In en, this message translates to:
  /// **'Created invitation'**
  String get createdInvitation;

  /// No description provided for @joinedInvitation.
  ///
  /// In en, this message translates to:
  /// **'Joined invitation'**
  String get joinedInvitation;

  /// No description provided for @notMeasured.
  ///
  /// In en, this message translates to:
  /// **'Not measured'**
  String get notMeasured;

  /// No description provided for @never.
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get never;

  /// No description provided for @presence.
  ///
  /// In en, this message translates to:
  /// **'Presence'**
  String get presence;

  /// No description provided for @lastSeen.
  ///
  /// In en, this message translates to:
  /// **'Last seen'**
  String get lastSeen;

  /// No description provided for @todayUpper.
  ///
  /// In en, this message translates to:
  /// **'TODAY'**
  String get todayUpper;

  /// No description provided for @sampleContactName.
  ///
  /// In en, this message translates to:
  /// **'Alice'**
  String get sampleContactName;

  /// No description provided for @sampleOnline.
  ///
  /// In en, this message translates to:
  /// **'online'**
  String get sampleOnline;

  /// No description provided for @sampleTime.
  ///
  /// In en, this message translates to:
  /// **'14:22'**
  String get sampleTime;

  /// No description provided for @searchMessages.
  ///
  /// In en, this message translates to:
  /// **'Search messages'**
  String get searchMessages;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @removeAttachment.
  ///
  /// In en, this message translates to:
  /// **'Remove attachment'**
  String get removeAttachment;

  /// No description provided for @scanQr.
  ///
  /// In en, this message translates to:
  /// **'Scan QR'**
  String get scanQr;

  /// No description provided for @contactDetails.
  ///
  /// In en, this message translates to:
  /// **'Contact details'**
  String get contactDetails;

  /// No description provided for @contactBlocked.
  ///
  /// In en, this message translates to:
  /// **'Contact is blocked'**
  String get contactBlocked;

  /// No description provided for @connecting.
  ///
  /// In en, this message translates to:
  /// **'Connecting'**
  String get connecting;

  /// No description provided for @reconnecting.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting'**
  String get reconnecting;

  /// No description provided for @peerOffline.
  ///
  /// In en, this message translates to:
  /// **'Peer is offline'**
  String get peerOffline;

  /// No description provided for @p2pShort.
  ///
  /// In en, this message translates to:
  /// **'P2P'**
  String get p2pShort;

  /// No description provided for @startingShort.
  ///
  /// In en, this message translates to:
  /// **'Starting'**
  String get startingShort;

  /// No description provided for @reconnectingShort.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting'**
  String get reconnectingShort;

  /// No description provided for @offlineShort.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get offlineShort;

  /// No description provided for @nativeBridge.
  ///
  /// In en, this message translates to:
  /// **'Native bridge'**
  String get nativeBridge;

  /// No description provided for @localIdentityCheck.
  ///
  /// In en, this message translates to:
  /// **'Local identity'**
  String get localIdentityCheck;

  /// No description provided for @directPeers.
  ///
  /// In en, this message translates to:
  /// **'Direct peers'**
  String get directPeers;

  /// No description provided for @noContactsPaired.
  ///
  /// In en, this message translates to:
  /// **'No contacts paired'**
  String get noContactsPaired;

  /// No description provided for @contractSnapshotReadable.
  ///
  /// In en, this message translates to:
  /// **'Contract snapshot readable'**
  String get contractSnapshotReadable;

  /// No description provided for @notInitialized.
  ///
  /// In en, this message translates to:
  /// **'Not initialized'**
  String get notInitialized;

  /// No description provided for @loaded.
  ///
  /// In en, this message translates to:
  /// **'Loaded'**
  String get loaded;

  /// No description provided for @published.
  ///
  /// In en, this message translates to:
  /// **'Published'**
  String get published;

  /// No description provided for @redactedHealthEventsReadable.
  ///
  /// In en, this message translates to:
  /// **'Redacted health events readable'**
  String get redactedHealthEventsReadable;

  /// No description provided for @noReadableHealthEvents.
  ///
  /// In en, this message translates to:
  /// **'No readable health events'**
  String get noReadableHealthEvents;

  /// No description provided for @startingSecureNetwork.
  ///
  /// In en, this message translates to:
  /// **'Starting secure network…'**
  String get startingSecureNetwork;

  /// No description provided for @blockContactTitle.
  ///
  /// In en, this message translates to:
  /// **'Block {name}?'**
  String blockContactTitle(Object name);

  /// No description provided for @removeContactTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}?'**
  String removeContactTitle(Object name);

  /// No description provided for @contactAddedToContacts.
  ///
  /// In en, this message translates to:
  /// **'{name} was added to Contacts'**
  String contactAddedToContacts(Object name);

  /// No description provided for @contactAcceptedJoin.
  ///
  /// In en, this message translates to:
  /// **'{name} accepted your join request'**
  String contactAcceptedJoin(Object name);

  /// No description provided for @buildTooltip.
  ///
  /// In en, this message translates to:
  /// **'Torca build {build}\\nProvider service: {service}'**
  String buildTooltip(Object build, Object service);

  /// No description provided for @buildLabel.
  ///
  /// In en, this message translates to:
  /// **'build {build}'**
  String buildLabel(Object build);

  /// No description provided for @attachmentsQueued.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 attachment queued} other{{count} attachments queued}}'**
  String attachmentsQueued(num count);

  /// No description provided for @contactsCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 private contact} other{{count} private contacts}}'**
  String contactsCount(num count);

  /// No description provided for @directPeerLinksReady.
  ///
  /// In en, this message translates to:
  /// **'{ready} of {total} direct peer links ready'**
  String directPeerLinksReady(Object ready, Object total);

  /// No description provided for @remoteIdentity.
  ///
  /// In en, this message translates to:
  /// **'Identity {id}'**
  String remoteIdentity(Object id);

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// No description provided for @appearanceTitle.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearanceTitle;

  /// No description provided for @languageTitle.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get languageTitle;

  /// No description provided for @systemLanguage.
  ///
  /// In en, this message translates to:
  /// **'System language'**
  String get systemLanguage;

  /// No description provided for @chooseLanguage.
  ///
  /// In en, this message translates to:
  /// **'Choose your language'**
  String get chooseLanguage;

  /// No description provided for @chooseNickname.
  ///
  /// In en, this message translates to:
  /// **'Choose your nickname'**
  String get chooseNickname;

  /// No description provided for @nickname.
  ///
  /// In en, this message translates to:
  /// **'Nickname'**
  String get nickname;

  /// No description provided for @continueLabel.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get continueLabel;

  /// No description provided for @privacyTitle.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get privacyTitle;

  /// No description provided for @notificationsTitle.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notificationsTitle;

  /// No description provided for @messageSenderYou.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get messageSenderYou;

  /// No description provided for @messageSenderContact.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get messageSenderContact;

  /// No description provided for @retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;
}

class _TorcaLocalizationsDelegate
    extends LocalizationsDelegate<TorcaLocalizations> {
  const _TorcaLocalizationsDelegate();

  @override
  Future<TorcaLocalizations> load(Locale locale) {
    return SynchronousFuture<TorcaLocalizations>(
      lookupTorcaLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) => <String>[
    'de',
    'en',
    'es',
    'fr',
    'pl',
    'uk',
  ].contains(locale.languageCode);

  @override
  bool shouldReload(_TorcaLocalizationsDelegate old) => false;
}

TorcaLocalizations lookupTorcaLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return TorcaLocalizationsDe();
    case 'en':
      return TorcaLocalizationsEn();
    case 'es':
      return TorcaLocalizationsEs();
    case 'fr':
      return TorcaLocalizationsFr();
    case 'pl':
      return TorcaLocalizationsPl();
    case 'uk':
      return TorcaLocalizationsUk();
  }

  throw FlutterError(
    'TorcaLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}

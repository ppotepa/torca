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

  /// No description provided for @aboutTorca.
  ///
  /// In en, this message translates to:
  /// **'About Torca'**
  String get aboutTorca;

  /// No description provided for @accept.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get accept;

  /// No description provided for @activeDemands.
  ///
  /// In en, this message translates to:
  /// **'Active demands'**
  String get activeDemands;

  /// No description provided for @activeInvitationsDescription.
  ///
  /// In en, this message translates to:
  /// **'Your active invitations and pairing requests will appear here.'**
  String get activeInvitationsDescription;

  /// No description provided for @activeLeases.
  ///
  /// In en, this message translates to:
  /// **'Active leases'**
  String get activeLeases;

  /// No description provided for @activeTransfers.
  ///
  /// In en, this message translates to:
  /// **'Active'**
  String get activeTransfers;

  /// No description provided for @allOperations.
  ///
  /// In en, this message translates to:
  /// **'All'**
  String get allOperations;

  /// No description provided for @allowAll.
  ///
  /// In en, this message translates to:
  /// **'Allow all'**
  String get allowAll;

  /// No description provided for @allowDelayedBackgroundDelivery.
  ///
  /// In en, this message translates to:
  /// **'Allow delayed background delivery'**
  String get allowDelayedBackgroundDelivery;

  /// No description provided for @allowDelayedBackgroundDeliveryDescription.
  ///
  /// In en, this message translates to:
  /// **'Required before Automatic or Saver can suspend the communication runtime while the app is idle.'**
  String get allowDelayedBackgroundDeliveryDescription;

  /// No description provided for @alwaysAvailable.
  ///
  /// In en, this message translates to:
  /// **'Always available'**
  String get alwaysAvailable;

  /// No description provided for @appearance.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearance;

  /// No description provided for @appearanceTitle.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearanceTitle;

  /// No description provided for @applicationMenu.
  ///
  /// In en, this message translates to:
  /// **'Application menu'**
  String get applicationMenu;

  /// No description provided for @archiveConversation.
  ///
  /// In en, this message translates to:
  /// **'Archive conversation'**
  String get archiveConversation;

  /// No description provided for @attachFiles.
  ///
  /// In en, this message translates to:
  /// **'Attach files'**
  String get attachFiles;

  /// No description provided for @attachmentAckTimeout.
  ///
  /// In en, this message translates to:
  /// **'waiting for peer acknowledgement'**
  String get attachmentAckTimeout;

  /// No description provided for @attachmentDependencyMissing.
  ///
  /// In en, this message translates to:
  /// **'waiting for conversation'**
  String get attachmentDependencyMissing;

  /// No description provided for @attachmentIntegrityFailed.
  ///
  /// In en, this message translates to:
  /// **'integrity check failed'**
  String get attachmentIntegrityFailed;

  /// No description provided for @attachmentMessagePending.
  ///
  /// In en, this message translates to:
  /// **'waiting for message'**
  String get attachmentMessagePending;

  /// No description provided for @attachmentOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'Attachment operation failed'**
  String get attachmentOperationFailed;

  /// No description provided for @attachmentPeerUnavailable.
  ///
  /// In en, this message translates to:
  /// **'peer unavailable'**
  String get attachmentPeerUnavailable;

  /// No description provided for @attachmentRetryAvailable.
  ///
  /// In en, this message translates to:
  /// **'retry available'**
  String get attachmentRetryAvailable;

  /// No description provided for @attachmentSaved.
  ///
  /// In en, this message translates to:
  /// **'Attachment saved'**
  String get attachmentSaved;

  /// No description provided for @attachmentStorageFailed.
  ///
  /// In en, this message translates to:
  /// **'local storage failed'**
  String get attachmentStorageFailed;

  /// No description provided for @attachmentSyncing.
  ///
  /// In en, this message translates to:
  /// **'Attachment is syncing…'**
  String get attachmentSyncing;

  /// No description provided for @attachmentsQueued.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 attachment queued} other{{count} attachments queued}}'**
  String attachmentsQueued(num count);

  /// No description provided for @audio.
  ///
  /// In en, this message translates to:
  /// **'Audio'**
  String get audio;

  /// No description provided for @audioDeviceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The selected audio device is unavailable.'**
  String get audioDeviceUnavailable;

  /// No description provided for @audioOutput.
  ///
  /// In en, this message translates to:
  /// **'Audio output'**
  String get audioOutput;

  /// No description provided for @automatic.
  ///
  /// In en, this message translates to:
  /// **'Automatic'**
  String get automatic;

  /// No description provided for @availabilityMode.
  ///
  /// In en, this message translates to:
  /// **'Availability mode'**
  String get availabilityMode;

  /// No description provided for @batteryAvailability.
  ///
  /// In en, this message translates to:
  /// **'Battery & availability'**
  String get batteryAvailability;

  /// No description provided for @batteryObservation.
  ///
  /// In en, this message translates to:
  /// **'Battery observation'**
  String get batteryObservation;

  /// No description provided for @batterySaver.
  ///
  /// In en, this message translates to:
  /// **'Battery saver'**
  String get batterySaver;

  /// No description provided for @batterySettingsDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose when Torca may defer background work. Incoming work is never silently discarded.'**
  String get batterySettingsDescription;

  /// No description provided for @batteryTab.
  ///
  /// In en, this message translates to:
  /// **'Battery'**
  String get batteryTab;

  /// No description provided for @blockContact.
  ///
  /// In en, this message translates to:
  /// **'Block contact'**
  String get blockContact;

  /// No description provided for @blockContactDescription.
  ///
  /// In en, this message translates to:
  /// **'Torca will close the peer connection and will not reconnect until you unblock this contact.'**
  String get blockContactDescription;

  /// No description provided for @blockContactTitle.
  ///
  /// In en, this message translates to:
  /// **'Block {name}?'**
  String blockContactTitle(Object name);

  /// No description provided for @blocked.
  ///
  /// In en, this message translates to:
  /// **'Blocked'**
  String get blocked;

  /// No description provided for @blockedSendBlocked.
  ///
  /// In en, this message translates to:
  /// **'This contact is blocked. Unblock the contact to send a message.'**
  String get blockedSendBlocked;

  /// No description provided for @bookmarkMessage.
  ///
  /// In en, this message translates to:
  /// **'Bookmark message'**
  String get bookmarkMessage;

  /// No description provided for @bootstrapAttempt.
  ///
  /// In en, this message translates to:
  /// **'{label} · attempt {attempt}'**
  String bootstrapAttempt(String label, int attempt);

  /// No description provided for @bootstrapProgress.
  ///
  /// In en, this message translates to:
  /// **'{ready} of {total} secure checks complete  •  {elapsed}'**
  String bootstrapProgress(int ready, int total, String elapsed);

  /// No description provided for @bootstrapStateDescription.
  ///
  /// In en, this message translates to:
  /// **'{id}: {value} {code}'**
  String bootstrapStateDescription(Object code, Object id, Object value);

  /// No description provided for @bootstrapStepLabel.
  ///
  /// In en, this message translates to:
  /// **'{id}'**
  String bootstrapStepLabel(Object id);

  /// No description provided for @build.
  ///
  /// In en, this message translates to:
  /// **'Build'**
  String get build;

  /// No description provided for @buildAndConnectionInfo.
  ///
  /// In en, this message translates to:
  /// **'Build & connection info'**
  String get buildAndConnectionInfo;

  /// No description provided for @buildLabel.
  ///
  /// In en, this message translates to:
  /// **'build {build}'**
  String buildLabel(Object build);

  /// No description provided for @buildServiceSummary.
  ///
  /// In en, this message translates to:
  /// **'{build} {service}'**
  String buildServiceSummary(Object build, Object service);

  /// No description provided for @buildTooltip.
  ///
  /// In en, this message translates to:
  /// **'Torca build {build}\nProvider service: {providerService}'**
  String buildTooltip(Object build, Object providerService);

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @cancelInvitation.
  ///
  /// In en, this message translates to:
  /// **'Cancel invitation'**
  String get cancelInvitation;

  /// No description provided for @cancelMessage.
  ///
  /// In en, this message translates to:
  /// **'Cancel message'**
  String get cancelMessage;

  /// No description provided for @cancelRequest.
  ///
  /// In en, this message translates to:
  /// **'Cancel request'**
  String get cancelRequest;

  /// No description provided for @cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get cancelled;

  /// No description provided for @chats.
  ///
  /// In en, this message translates to:
  /// **'Chats'**
  String get chats;

  /// No description provided for @checkingInvitation.
  ///
  /// In en, this message translates to:
  /// **'Checking invitation...'**
  String get checkingInvitation;

  /// No description provided for @chooseConversation.
  ///
  /// In en, this message translates to:
  /// **'Choose conversation'**
  String get chooseConversation;

  /// No description provided for @chooseLanguage.
  ///
  /// In en, this message translates to:
  /// **'Choose your language'**
  String get chooseLanguage;

  /// No description provided for @chooseLanguagePolish.
  ///
  /// In en, this message translates to:
  /// **'Choose Language Polish'**
  String get chooseLanguagePolish;

  /// No description provided for @chooseNickname.
  ///
  /// In en, this message translates to:
  /// **'Choose your nickname'**
  String get chooseNickname;

  /// No description provided for @clearConversationHistory.
  ///
  /// In en, this message translates to:
  /// **'Clear conversation history'**
  String get clearConversationHistory;

  /// No description provided for @clearSearch.
  ///
  /// In en, this message translates to:
  /// **'Clear search'**
  String get clearSearch;

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @closeInvitationDescription.
  ///
  /// In en, this message translates to:
  /// **'Close this window to continue using the application. The invitation will appear here automatically when the connection is ready.'**
  String get closeInvitationDescription;

  /// No description provided for @closeScanner.
  ///
  /// In en, this message translates to:
  /// **'Close scanner'**
  String get closeScanner;

  /// No description provided for @closeSearch.
  ///
  /// In en, this message translates to:
  /// **'Close search'**
  String get closeSearch;

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

  /// No description provided for @closeTooltip.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get closeTooltip;

  /// No description provided for @collapseNavigation.
  ///
  /// In en, this message translates to:
  /// **'Collapse navigation'**
  String get collapseNavigation;

  /// No description provided for @comfortableDensity.
  ///
  /// In en, this message translates to:
  /// **'Comfortable density'**
  String get comfortableDensity;

  /// No description provided for @communicationProvider.
  ///
  /// In en, this message translates to:
  /// **'Communication provider'**
  String get communicationProvider;

  /// No description provided for @communicationState.
  ///
  /// In en, this message translates to:
  /// **'Communication state'**
  String get communicationState;

  /// No description provided for @compactDensity.
  ///
  /// In en, this message translates to:
  /// **'Compact density'**
  String get compactDensity;

  /// No description provided for @completedTransfers.
  ///
  /// In en, this message translates to:
  /// **'Completed'**
  String get completedTransfers;

  /// No description provided for @connecting.
  ///
  /// In en, this message translates to:
  /// **'Connecting'**
  String get connecting;

  /// No description provided for @connectingPeerThrough.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Connecting to peer through Iroh} memory {Connecting to peer through Memory test} other {Connecting to peer through {provider}}}'**
  String connectingPeerThrough(String provider);

  /// No description provided for @connection.
  ///
  /// In en, this message translates to:
  /// **'Connection'**
  String get connection;

  /// No description provided for @connectionDetails.
  ///
  /// In en, this message translates to:
  /// **'Connection details'**
  String get connectionDetails;

  /// No description provided for @connectionDetailsTitle.
  ///
  /// In en, this message translates to:
  /// **'Connection details'**
  String get connectionDetailsTitle;

  /// No description provided for @connectionEvidenceNote.
  ///
  /// In en, this message translates to:
  /// **'Quality describes the authenticated direct peer link over {provider}. It is runtime evidence, not radio or internet signal strength.'**
  String connectionEvidenceNote(String provider);

  /// No description provided for @connectionQuality.
  ///
  /// In en, this message translates to:
  /// **'Connection quality {quality}{rtt}'**
  String connectionQuality(Object quality, Object rtt);

  /// No description provided for @connectionSelfTest.
  ///
  /// In en, this message translates to:
  /// **'Connection self-test'**
  String get connectionSelfTest;

  /// No description provided for @consecutiveFailures.
  ///
  /// In en, this message translates to:
  /// **'Consecutive failures'**
  String get consecutiveFailures;

  /// No description provided for @contactAcceptedJoin.
  ///
  /// In en, this message translates to:
  /// **'{name} accepted your join request'**
  String contactAcceptedJoin(Object name);

  /// No description provided for @contactActions.
  ///
  /// In en, this message translates to:
  /// **'Contact actions'**
  String get contactActions;

  /// No description provided for @contactAddedToContacts.
  ///
  /// In en, this message translates to:
  /// **'{name} was added to Contacts'**
  String contactAddedToContacts(Object name);

  /// No description provided for @contactBlocked.
  ///
  /// In en, this message translates to:
  /// **'Contact is blocked'**
  String get contactBlocked;

  /// No description provided for @contactConnected.
  ///
  /// In en, this message translates to:
  /// **'Contact connected'**
  String get contactConnected;

  /// No description provided for @contactConnectedDescription.
  ///
  /// In en, this message translates to:
  /// **'The invitation was accepted and this contact is ready to chat.'**
  String get contactConnectedDescription;

  /// No description provided for @contactDetails.
  ///
  /// In en, this message translates to:
  /// **'Contact details'**
  String get contactDetails;

  /// No description provided for @contactInformation.
  ///
  /// In en, this message translates to:
  /// **'Contact information'**
  String get contactInformation;

  /// No description provided for @contactLabel.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get contactLabel;

  /// No description provided for @contactUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This contact is no longer available.'**
  String get contactUnavailable;

  /// No description provided for @contacts.
  ///
  /// In en, this message translates to:
  /// **'Contacts'**
  String get contacts;

  /// No description provided for @contactsCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 private contact} other{{count} private contacts}}'**
  String contactsCount(num count);

  /// No description provided for @continueLabel.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get continueLabel;

  /// No description provided for @contract.
  ///
  /// In en, this message translates to:
  /// **'Contract'**
  String get contract;

  /// No description provided for @contractDecodeFailed.
  ///
  /// In en, this message translates to:
  /// **'The client and native runtime use incompatible data. Rebuild and redeploy both.'**
  String get contractDecodeFailed;

  /// No description provided for @contractSnapshotReadable.
  ///
  /// In en, this message translates to:
  /// **'Contract snapshot readable'**
  String get contractSnapshotReadable;

  /// No description provided for @conversationActions.
  ///
  /// In en, this message translates to:
  /// **'Conversation actions'**
  String get conversationActions;

  /// No description provided for @copy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copy;

  /// No description provided for @copyCode.
  ///
  /// In en, this message translates to:
  /// **'Copy invitation'**
  String get copyCode;

  /// No description provided for @copyFingerprint.
  ///
  /// In en, this message translates to:
  /// **'Copy fingerprint'**
  String get copyFingerprint;

  /// No description provided for @couldNotBlockContact.
  ///
  /// In en, this message translates to:
  /// **'Could not block contact'**
  String get couldNotBlockContact;

  /// No description provided for @couldNotForwardMessage.
  ///
  /// In en, this message translates to:
  /// **'Could not forward message'**
  String get couldNotForwardMessage;

  /// No description provided for @couldNotQueueAttachment.
  ///
  /// In en, this message translates to:
  /// **'Could not queue attachment'**
  String get couldNotQueueAttachment;

  /// No description provided for @couldNotRemoveContact.
  ///
  /// In en, this message translates to:
  /// **'Could not remove contact'**
  String get couldNotRemoveContact;

  /// No description provided for @couldNotRenameContact.
  ///
  /// In en, this message translates to:
  /// **'Could not rename contact'**
  String get couldNotRenameContact;

  /// No description provided for @couldNotSaveNickname.
  ///
  /// In en, this message translates to:
  /// **'Could not save nickname'**
  String get couldNotSaveNickname;

  /// No description provided for @couldNotStartConversation.
  ///
  /// In en, this message translates to:
  /// **'Could not start conversation with {name}.'**
  String couldNotStartConversation(Object name);

  /// No description provided for @couldNotStartRadio.
  ///
  /// In en, this message translates to:
  /// **'Could not start transmission'**
  String get couldNotStartRadio;

  /// No description provided for @couldNotUnblockContact.
  ///
  /// In en, this message translates to:
  /// **'Could not unblock contact'**
  String get couldNotUnblockContact;

  /// No description provided for @couldNotUpdateRadio.
  ///
  /// In en, this message translates to:
  /// **'Could not update Radio mode'**
  String get couldNotUpdateRadio;

  /// No description provided for @couldNotUpdateReaction.
  ///
  /// In en, this message translates to:
  /// **'Could not send reaction'**
  String get couldNotUpdateReaction;

  /// No description provided for @country.
  ///
  /// In en, this message translates to:
  /// **'Where are you from?'**
  String get country;

  /// No description provided for @createInvitationForContact.
  ///
  /// In en, this message translates to:
  /// **'Create an invitation to add a private contact.'**
  String get createInvitationForContact;

  /// No description provided for @createManageInvitations.
  ///
  /// In en, this message translates to:
  /// **'Create and manage short-lived private contact invitations.'**
  String get createManageInvitations;

  /// No description provided for @createdInvitation.
  ///
  /// In en, this message translates to:
  /// **'Created invitation'**
  String get createdInvitation;

  /// No description provided for @dark.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get dark;

  /// No description provided for @defaultAudioDevice.
  ///
  /// In en, this message translates to:
  /// **'{name} (default)'**
  String defaultAudioDevice(Object name);

  /// No description provided for @deleteMessage.
  ///
  /// In en, this message translates to:
  /// **'Delete for everyone'**
  String get deleteMessage;

  /// No description provided for @deleteMessageTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete message?'**
  String get deleteMessageTitle;

  /// No description provided for @delivered.
  ///
  /// In en, this message translates to:
  /// **'Delivered'**
  String get delivered;

  /// No description provided for @deliveryFailed.
  ///
  /// In en, this message translates to:
  /// **'Delivery failed'**
  String get deliveryFailed;

  /// No description provided for @desktop.
  ///
  /// In en, this message translates to:
  /// **'Desktop'**
  String get desktop;

  /// No description provided for @deviceFingerprint.
  ///
  /// In en, this message translates to:
  /// **'Device fingerprint\n{fingerprint}'**
  String deviceFingerprint(Object fingerprint);

  /// No description provided for @diagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get diagnostics;

  /// No description provided for @diagnosticsExported.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics exported'**
  String get diagnosticsExported;

  /// No description provided for @diagnosticsStream.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics stream'**
  String get diagnosticsStream;

  /// No description provided for @directPeerLinksReady.
  ///
  /// In en, this message translates to:
  /// **'{ready} of {total} direct peer links ready'**
  String directPeerLinksReady(Object ready, Object total);

  /// No description provided for @directPeers.
  ///
  /// In en, this message translates to:
  /// **'Direct peers'**
  String get directPeers;

  /// No description provided for @directProviderContact.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Direct Iroh contact} memory {Direct Memory test contact} other {Direct {provider} contact}}'**
  String directProviderContact(String provider);

  /// No description provided for @displayName.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get displayName;

  /// No description provided for @documentTransfers.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get documentTransfers;

  /// No description provided for @done.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @draft.
  ///
  /// In en, this message translates to:
  /// **'Draft'**
  String get draft;

  /// No description provided for @editMessage.
  ///
  /// In en, this message translates to:
  /// **'Edit message'**
  String get editMessage;

  /// No description provided for @emoji.
  ///
  /// In en, this message translates to:
  /// **'Emoji'**
  String get emoji;

  /// No description provided for @enableNotifications.
  ///
  /// In en, this message translates to:
  /// **'Enable notifications'**
  String get enableNotifications;

  /// No description provided for @encrypting.
  ///
  /// In en, this message translates to:
  /// **'Encrypting'**
  String get encrypting;

  /// No description provided for @endpoint.
  ///
  /// In en, this message translates to:
  /// **'Endpoint'**
  String get endpoint;

  /// No description provided for @englishCountry.
  ///
  /// In en, this message translates to:
  /// **'England'**
  String get englishCountry;

  /// No description provided for @enterSixCharacterCode.
  ///
  /// In en, this message translates to:
  /// **'Enter a six-character code or scan the QR code.'**
  String get enterSixCharacterCode;

  /// No description provided for @excellent.
  ///
  /// In en, this message translates to:
  /// **'Excellent'**
  String get excellent;

  /// No description provided for @expandNavigation.
  ///
  /// In en, this message translates to:
  /// **'Expand navigation'**
  String get expandNavigation;

  /// No description provided for @exportDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Export diagnostics'**
  String get exportDiagnostics;

  /// No description provided for @exportFailed.
  ///
  /// In en, this message translates to:
  /// **'Export failed'**
  String get exportFailed;

  /// No description provided for @exportTorcaDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Export Torca diagnostics'**
  String get exportTorcaDiagnostics;

  /// No description provided for @fair.
  ///
  /// In en, this message translates to:
  /// **'Fair'**
  String get fair;

  /// No description provided for @fileTransfers.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get fileTransfers;

  /// No description provided for @finalizingContact.
  ///
  /// In en, this message translates to:
  /// **'Finalizing secure contact…'**
  String get finalizingContact;

  /// No description provided for @fingerprint.
  ///
  /// In en, this message translates to:
  /// **'Fingerprint'**
  String get fingerprint;

  /// No description provided for @fingerprintCopied.
  ///
  /// In en, this message translates to:
  /// **'Fingerprint copied'**
  String get fingerprintCopied;

  /// No description provided for @focusedOnly.
  ///
  /// In en, this message translates to:
  /// **'Animate focused views'**
  String get focusedOnly;

  /// No description provided for @followSystem.
  ///
  /// In en, this message translates to:
  /// **'Follow system setting'**
  String get followSystem;

  /// No description provided for @forwardMessage.
  ///
  /// In en, this message translates to:
  /// **'Forward message'**
  String get forwardMessage;

  /// No description provided for @forwardNoAvailableAttachments.
  ///
  /// In en, this message translates to:
  /// **'{count}'**
  String forwardNoAvailableAttachments(Object count);

  /// No description provided for @forwardSkippedAttachments.
  ///
  /// In en, this message translates to:
  /// **'{count}'**
  String forwardSkippedAttachments(Object count);

  /// No description provided for @fullAnimation.
  ///
  /// In en, this message translates to:
  /// **'Full animation'**
  String get fullAnimation;

  /// No description provided for @generateInvitation.
  ///
  /// In en, this message translates to:
  /// **'Generate Invitation'**
  String get generateInvitation;

  /// No description provided for @generatingInvitation.
  ///
  /// In en, this message translates to:
  /// **'Generating…'**
  String get generatingInvitation;

  /// No description provided for @good.
  ///
  /// In en, this message translates to:
  /// **'Good'**
  String get good;

  /// No description provided for @holdToRecordVoiceClip.
  ///
  /// In en, this message translates to:
  /// **'Hold to record a voice clip'**
  String get holdToRecordVoiceClip;

  /// No description provided for @identicalDeadlineReplacements.
  ///
  /// In en, this message translates to:
  /// **'Identical deadline replacements'**
  String get identicalDeadlineReplacements;

  /// No description provided for @identity.
  ///
  /// In en, this message translates to:
  /// **'Identity'**
  String get identity;

  /// No description provided for @identityChanged.
  ///
  /// In en, this message translates to:
  /// **'The contact identity changed. Verify the Safety Number.'**
  String get identityChanged;

  /// No description provided for @identityChangedSendBlocked.
  ///
  /// In en, this message translates to:
  /// **'Sending is paused until this contact is verified again.'**
  String get identityChangedSendBlocked;

  /// No description provided for @incidentDescription.
  ///
  /// In en, this message translates to:
  /// **'Run a self-test, mark the current state and export the redacted snapshot. Message text, attachments, audio and secrets are not included.'**
  String get incidentDescription;

  /// No description provided for @incidentSnapshotSaved.
  ///
  /// In en, this message translates to:
  /// **'Incident snapshot saved to this run\'s local diagnostics.'**
  String get incidentSnapshotSaved;

  /// No description provided for @incidentTab.
  ///
  /// In en, this message translates to:
  /// **'Incident'**
  String get incidentTab;

  /// No description provided for @incidentTools.
  ///
  /// In en, this message translates to:
  /// **'Incident tools'**
  String get incidentTools;

  /// No description provided for @incomingMessage.
  ///
  /// In en, this message translates to:
  /// **'Incoming message'**
  String get incomingMessage;

  /// No description provided for @incompatibleStorageEpoch.
  ///
  /// In en, this message translates to:
  /// **'The encrypted local profile is incompatible. Reset local Torca data explicitly before continuing.'**
  String get incompatibleStorageEpoch;

  /// No description provided for @instantMode.
  ///
  /// In en, this message translates to:
  /// **'Instant mode'**
  String get instantMode;

  /// No description provided for @instantModeEnabled.
  ///
  /// In en, this message translates to:
  /// **'Instant mode enabled'**
  String get instantModeEnabled;

  /// No description provided for @invalidInput.
  ///
  /// In en, this message translates to:
  /// **'The supplied value is not valid.'**
  String get invalidInput;

  /// No description provided for @invitationCode.
  ///
  /// In en, this message translates to:
  /// **'Invitation code'**
  String get invitationCode;

  /// No description provided for @invitationCodeCopied.
  ///
  /// In en, this message translates to:
  /// **'Full invitation copied'**
  String get invitationCodeCopied;

  /// No description provided for @invitationCodeLabel.
  ///
  /// In en, this message translates to:
  /// **'Code {code}'**
  String invitationCodeLabel(Object code);

  /// No description provided for @invitationExpiresIn.
  ///
  /// In en, this message translates to:
  /// **'Expires in {countdown}'**
  String invitationExpiresIn(Object countdown);

  /// No description provided for @invitationGenerating.
  ///
  /// In en, this message translates to:
  /// **'Generating a private invitation...'**
  String get invitationGenerating;

  /// No description provided for @invitationJoinSent.
  ///
  /// In en, this message translates to:
  /// **'Join request sent. You will be notified when it is accepted.'**
  String get invitationJoinSent;

  /// No description provided for @invitationOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'Invitation operation failed'**
  String get invitationOperationFailed;

  /// No description provided for @invitationQueued.
  ///
  /// In en, this message translates to:
  /// **'Invitation queued for the secure network.'**
  String get invitationQueued;

  /// No description provided for @invitationSavedLocally.
  ///
  /// In en, this message translates to:
  /// **'Saved locally. It will retry when the selected communication provider is ready.'**
  String get invitationSavedLocally;

  /// No description provided for @invitationWaitingForNetwork.
  ///
  /// In en, this message translates to:
  /// **'Invitation is waiting for the network.'**
  String get invitationWaitingForNetwork;

  /// No description provided for @invitations.
  ///
  /// In en, this message translates to:
  /// **'Invitations'**
  String get invitations;

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

  /// No description provided for @joinInvitation.
  ///
  /// In en, this message translates to:
  /// **'Join invitation'**
  String get joinInvitation;

  /// No description provided for @joinRequestWaiting.
  ///
  /// In en, this message translates to:
  /// **'Your request is waiting for the invitation owner to verify and accept it.'**
  String get joinRequestWaiting;

  /// No description provided for @joinedInvitation.
  ///
  /// In en, this message translates to:
  /// **'Joined invitation'**
  String get joinedInvitation;

  /// No description provided for @jumpToLatest.
  ///
  /// In en, this message translates to:
  /// **'Jump to latest message'**
  String get jumpToLatest;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

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

  /// No description provided for @languageSystem.
  ///
  /// In en, this message translates to:
  /// **'System language'**
  String get languageSystem;

  /// No description provided for @languageTitle.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get languageTitle;

  /// No description provided for @lastSeen.
  ///
  /// In en, this message translates to:
  /// **'Last seen'**
  String get lastSeen;

  /// No description provided for @lastSeenAt.
  ///
  /// In en, this message translates to:
  /// **'Last seen {time}'**
  String lastSeenAt(Object time);

  /// No description provided for @lastSuccessfulProbe.
  ///
  /// In en, this message translates to:
  /// **'Last successful probe'**
  String get lastSuccessfulProbe;

  /// No description provided for @leaseReasons.
  ///
  /// In en, this message translates to:
  /// **'Lease reasons'**
  String get leaseReasons;

  /// No description provided for @light.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get light;

  /// No description provided for @loadCurrentRunLogs.
  ///
  /// In en, this message translates to:
  /// **'Load current run logs'**
  String get loadCurrentRunLogs;

  /// No description provided for @loaded.
  ///
  /// In en, this message translates to:
  /// **'Loaded'**
  String get loaded;

  /// No description provided for @localIdentity.
  ///
  /// In en, this message translates to:
  /// **'Local identity'**
  String get localIdentity;

  /// No description provided for @localIdentityCheck.
  ///
  /// In en, this message translates to:
  /// **'Local identity'**
  String get localIdentityCheck;

  /// No description provided for @localIdentityNotReady.
  ///
  /// In en, this message translates to:
  /// **'Local identity is not ready'**
  String get localIdentityNotReady;

  /// No description provided for @localName.
  ///
  /// In en, this message translates to:
  /// **'Local name'**
  String get localName;

  /// No description provided for @logsTab.
  ///
  /// In en, this message translates to:
  /// **'Logs'**
  String get logsTab;

  /// No description provided for @markConversationRead.
  ///
  /// In en, this message translates to:
  /// **'Mark as read'**
  String get markConversationRead;

  /// No description provided for @markIncident.
  ///
  /// In en, this message translates to:
  /// **'Mark incident'**
  String get markIncident;

  /// No description provided for @mediaTransfers.
  ///
  /// In en, this message translates to:
  /// **'Media'**
  String get mediaTransfers;

  /// No description provided for @message.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get message;

  /// No description provided for @messageActions.
  ///
  /// In en, this message translates to:
  /// **'Message actions'**
  String get messageActions;

  /// No description provided for @messageCancelled.
  ///
  /// In en, this message translates to:
  /// **'Message cancelled'**
  String get messageCancelled;

  /// No description provided for @messageCopied.
  ///
  /// In en, this message translates to:
  /// **'Message copied'**
  String get messageCopied;

  /// No description provided for @messageDeleted.
  ///
  /// In en, this message translates to:
  /// **'Message deleted'**
  String get messageDeleted;

  /// No description provided for @messageDetails.
  ///
  /// In en, this message translates to:
  /// **'Message details'**
  String get messageDetails;

  /// No description provided for @messageEdited.
  ///
  /// In en, this message translates to:
  /// **'Message edited'**
  String get messageEdited;

  /// No description provided for @messageForwarded.
  ///
  /// In en, this message translates to:
  /// **'Message forwarded'**
  String get messageForwarded;

  /// No description provided for @messageQueued.
  ///
  /// In en, this message translates to:
  /// **'Queued — waiting for a direct peer connection'**
  String get messageQueued;

  /// No description provided for @messageSenderContact.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get messageSenderContact;

  /// No description provided for @messageSenderYou.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get messageSenderYou;

  /// No description provided for @messageTooLong.
  ///
  /// In en, this message translates to:
  /// **'Messages can contain at most {maximum} characters.'**
  String messageTooLong(int maximum);

  /// No description provided for @meteredTransfers.
  ///
  /// In en, this message translates to:
  /// **'Metered network transfers'**
  String get meteredTransfers;

  /// No description provided for @microphone.
  ///
  /// In en, this message translates to:
  /// **'Microphone'**
  String get microphone;

  /// No description provided for @microphonePermissionRequired.
  ///
  /// In en, this message translates to:
  /// **'Microphone access is required to transmit.'**
  String get microphonePermissionRequired;

  /// No description provided for @modern.
  ///
  /// In en, this message translates to:
  /// **'Modern'**
  String get modern;

  /// No description provided for @muteConversation.
  ///
  /// In en, this message translates to:
  /// **'Mute conversation'**
  String get muteConversation;

  /// No description provided for @nativeBridge.
  ///
  /// In en, this message translates to:
  /// **'Native bridge'**
  String get nativeBridge;

  /// No description provided for @nativeLogTails.
  ///
  /// In en, this message translates to:
  /// **'Native log tails'**
  String get nativeLogTails;

  /// No description provided for @nativeLogTailsDescription.
  ///
  /// In en, this message translates to:
  /// **'Loads a bounded, redacted tail from current-run native logs only. This explicit read does not keep a watcher alive.'**
  String get nativeLogTailsDescription;

  /// No description provided for @networkUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The selected communication connection is currently unavailable.'**
  String get networkUnavailable;

  /// No description provided for @never.
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get never;

  /// No description provided for @newContact.
  ///
  /// In en, this message translates to:
  /// **'New contact'**
  String get newContact;

  /// No description provided for @newDevice.
  ///
  /// In en, this message translates to:
  /// **'New device'**
  String get newDevice;

  /// No description provided for @newMessages.
  ///
  /// In en, this message translates to:
  /// **'New messages'**
  String get newMessages;

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

  /// No description provided for @newPrivateMessage.
  ///
  /// In en, this message translates to:
  /// **'New private message'**
  String get newPrivateMessage;

  /// No description provided for @nextDeadline.
  ///
  /// In en, this message translates to:
  /// **'Next deadline'**
  String get nextDeadline;

  /// No description provided for @nickname.
  ///
  /// In en, this message translates to:
  /// **'Nickname'**
  String get nickname;

  /// No description provided for @nicknameIntro.
  ///
  /// In en, this message translates to:
  /// **'The selected communication provider is ready. This name will be shown to contacts.'**
  String get nicknameIntro;

  /// No description provided for @nicknameRequired.
  ///
  /// In en, this message translates to:
  /// **'Nickname is required'**
  String get nicknameRequired;

  /// No description provided for @noActiveTransfers.
  ///
  /// In en, this message translates to:
  /// **'No active transfers.'**
  String get noActiveTransfers;

  /// No description provided for @noChatsMatch.
  ///
  /// In en, this message translates to:
  /// **'No chats match your search'**
  String get noChatsMatch;

  /// No description provided for @noContactsPaired.
  ///
  /// In en, this message translates to:
  /// **'No contacts paired'**
  String get noContactsPaired;

  /// No description provided for @noContactsYet.
  ///
  /// In en, this message translates to:
  /// **'No contacts yet'**
  String get noContactsYet;

  /// No description provided for @noForwardableContent.
  ///
  /// In en, this message translates to:
  /// **'This message has no content that can be forwarded.'**
  String get noForwardableContent;

  /// No description provided for @noInvitations.
  ///
  /// In en, this message translates to:
  /// **'No invitations'**
  String get noInvitations;

  /// No description provided for @noMatchingMessages.
  ///
  /// In en, this message translates to:
  /// **'No matching messages.'**
  String get noMatchingMessages;

  /// No description provided for @noMessagesYet.
  ///
  /// In en, this message translates to:
  /// **'No messages yet'**
  String get noMessagesYet;

  /// No description provided for @noMessagesYetDescription.
  ///
  /// In en, this message translates to:
  /// **'Messages are sent directly through the selected communication provider.'**
  String get noMessagesYetDescription;

  /// No description provided for @noReadableHealthEvents.
  ///
  /// In en, this message translates to:
  /// **'No readable health events'**
  String get noReadableHealthEvents;

  /// No description provided for @notInitialized.
  ///
  /// In en, this message translates to:
  /// **'Not initialized'**
  String get notInitialized;

  /// No description provided for @notMeasured.
  ///
  /// In en, this message translates to:
  /// **'Not measured'**
  String get notMeasured;

  /// No description provided for @notificationPrivacy.
  ///
  /// In en, this message translates to:
  /// **'Show private-message notifications without message content.'**
  String get notificationPrivacy;

  /// No description provided for @notifications.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notifications;

  /// No description provided for @notificationsTitle.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notificationsTitle;

  /// No description provided for @observationRecording.
  ///
  /// In en, this message translates to:
  /// **'recording'**
  String get observationRecording;

  /// No description provided for @observationRecordingDescription.
  ///
  /// In en, this message translates to:
  /// **'Recording deltas since the observation baseline.'**
  String get observationRecordingDescription;

  /// No description provided for @observationState.
  ///
  /// In en, this message translates to:
  /// **'State'**
  String get observationState;

  /// No description provided for @observationStopped.
  ///
  /// In en, this message translates to:
  /// **'stopped'**
  String get observationStopped;

  /// No description provided for @observationStoppedDescription.
  ///
  /// In en, this message translates to:
  /// **'Start before an idle or recovery scenario to record only new work.'**
  String get observationStoppedDescription;

  /// No description provided for @observationWork.
  ///
  /// In en, this message translates to:
  /// **'Work'**
  String get observationWork;

  /// No description provided for @offlineShort.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get offlineShort;

  /// No description provided for @online.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get online;

  /// No description provided for @open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// No description provided for @openChat.
  ///
  /// In en, this message translates to:
  /// **'Open chat'**
  String get openChat;

  /// No description provided for @openConversation.
  ///
  /// In en, this message translates to:
  /// **'Open conversation'**
  String get openConversation;

  /// No description provided for @operationFailed.
  ///
  /// In en, this message translates to:
  /// **'The operation could not be completed.'**
  String get operationFailed;

  /// No description provided for @originalMessageUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Original message unavailable'**
  String get originalMessageUnavailable;

  /// No description provided for @outgoingMessage.
  ///
  /// In en, this message translates to:
  /// **'Outgoing message'**
  String get outgoingMessage;

  /// No description provided for @p2pShort.
  ///
  /// In en, this message translates to:
  /// **'P2P'**
  String get p2pShort;

  /// No description provided for @pairContact.
  ///
  /// In en, this message translates to:
  /// **'Pair contact'**
  String get pairContact;

  /// No description provided for @pairContactHint.
  ///
  /// In en, this message translates to:
  /// **'Pair a contact to start a conversation.'**
  String get pairContactHint;

  /// No description provided for @pairingBootstrapRequired.
  ///
  /// In en, this message translates to:
  /// **'For this provider, scan the QR code or paste the full invitation link.'**
  String get pairingBootstrapRequired;

  /// No description provided for @pairingCompletedMessage.
  ///
  /// In en, this message translates to:
  /// **'The contact was added securely. Open the private conversation now.'**
  String get pairingCompletedMessage;

  /// No description provided for @pairingExpired.
  ///
  /// In en, this message translates to:
  /// **'The pairing invitation has expired.'**
  String get pairingExpired;

  /// No description provided for @pairingInactiveMessage.
  ///
  /// In en, this message translates to:
  /// **'This invitation is no longer active. The other device will receive the same final state.'**
  String get pairingInactiveMessage;

  /// No description provided for @pairingProviderMismatch.
  ///
  /// In en, this message translates to:
  /// **'This invitation belongs to a different communication provider.'**
  String get pairingProviderMismatch;

  /// No description provided for @pairingQrSemanticLabel.
  ///
  /// In en, this message translates to:
  /// **'Torca pairing invitation QR code'**
  String get pairingQrSemanticLabel;

  /// No description provided for @pairingRequestDescription.
  ///
  /// In en, this message translates to:
  /// **'This device joined your invitation. Review the contact details before accepting.'**
  String get pairingRequestDescription;

  /// No description provided for @pairingStateLabel.
  ///
  /// In en, this message translates to:
  /// **'{state, select, open {Open} peer_joined {Peer joined} awaiting_approval {Awaiting approval} approved {Approved} completed {Completed} rejected {Rejected} cancelled {Cancelled} expired {Expired} unknown {Unknown} other {Unknown}}'**
  String pairingStateLabel(String state);

  /// No description provided for @pauseAll.
  ///
  /// In en, this message translates to:
  /// **'Pause all transfers'**
  String get pauseAll;

  /// No description provided for @pauseLarge.
  ///
  /// In en, this message translates to:
  /// **'Pause large files'**
  String get pauseLarge;

  /// No description provided for @peerOffline.
  ///
  /// In en, this message translates to:
  /// **'Peer is offline'**
  String get peerOffline;

  /// No description provided for @peerState.
  ///
  /// In en, this message translates to:
  /// **'P2P state'**
  String get peerState;

  /// No description provided for @pendingOperations.
  ///
  /// In en, this message translates to:
  /// **'Pending'**
  String get pendingOperations;

  /// No description provided for @pinConversation.
  ///
  /// In en, this message translates to:
  /// **'Pin conversation'**
  String get pinConversation;

  /// No description provided for @playVoiceMessage.
  ///
  /// In en, this message translates to:
  /// **'Play voice message'**
  String get playVoiceMessage;

  /// No description provided for @polishCountry.
  ///
  /// In en, this message translates to:
  /// **'Poland'**
  String get polishCountry;

  /// No description provided for @poor.
  ///
  /// In en, this message translates to:
  /// **'Poor'**
  String get poor;

  /// No description provided for @preparingDownload.
  ///
  /// In en, this message translates to:
  /// **'Preparing download'**
  String get preparingDownload;

  /// No description provided for @preparingPrivateSpace.
  ///
  /// In en, this message translates to:
  /// **'Preparing your private space'**
  String get preparingPrivateSpace;

  /// No description provided for @preparingPrivateSpaceDescription.
  ///
  /// In en, this message translates to:
  /// **'Setting up encrypted storage and secure communication. You can safely leave this screen open.'**
  String get preparingPrivateSpaceDescription;

  /// No description provided for @preparingSecureCopy.
  ///
  /// In en, this message translates to:
  /// **'Preparing secure copy'**
  String get preparingSecureCopy;

  /// No description provided for @preparingUpload.
  ///
  /// In en, this message translates to:
  /// **'Preparing upload'**
  String get preparingUpload;

  /// No description provided for @presence.
  ///
  /// In en, this message translates to:
  /// **'Presence'**
  String get presence;

  /// No description provided for @privacy.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get privacy;

  /// No description provided for @privacyTitle.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get privacyTitle;

  /// No description provided for @productVersion.
  ///
  /// In en, this message translates to:
  /// **'Product version'**
  String get productVersion;

  /// No description provided for @profileNotReady.
  ///
  /// In en, this message translates to:
  /// **'The secure profile is not ready yet.'**
  String get profileNotReady;

  /// No description provided for @providerEndpoint.
  ///
  /// In en, this message translates to:
  /// **'Provider endpoint'**
  String get providerEndpoint;

  /// No description provided for @providerEndpointAvailable.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get providerEndpointAvailable;

  /// No description provided for @providerEndpointUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get providerEndpointUnavailable;

  /// No description provided for @providerName.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Iroh} memory {Memory test} other {{provider}}}'**
  String providerName(String provider);

  /// No description provided for @providerReady.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Iroh ready} memory {Memory test ready} other {{provider} ready}}'**
  String providerReady(String provider);

  /// No description provided for @providerReconnecting.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Iroh reconnecting} memory {Memory test reconnecting} other {{provider} reconnecting}}'**
  String providerReconnecting(String provider);

  /// No description provided for @providerStarting.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Iroh starting} memory {Memory test starting} other {{provider} starting}}'**
  String providerStarting(String provider);

  /// No description provided for @providerStateLabel.
  ///
  /// In en, this message translates to:
  /// **'{provider}: {state}'**
  String providerStateLabel(String provider, String state);

  /// No description provided for @published.
  ///
  /// In en, this message translates to:
  /// **'Published'**
  String get published;

  /// No description provided for @quality.
  ///
  /// In en, this message translates to:
  /// **'Quality'**
  String get quality;

  /// No description provided for @queued.
  ///
  /// In en, this message translates to:
  /// **'Queued'**
  String get queued;

  /// No description provided for @radioChannelInterrupted.
  ///
  /// In en, this message translates to:
  /// **'Radio channel was interrupted'**
  String get radioChannelInterrupted;

  /// No description provided for @radioChannelReady.
  ///
  /// In en, this message translates to:
  /// **'Private Radio channel is ready'**
  String get radioChannelReady;

  /// No description provided for @radioChannelRestored.
  ///
  /// In en, this message translates to:
  /// **'Radio channel was restored'**
  String get radioChannelRestored;

  /// No description provided for @radioConnecting.
  ///
  /// In en, this message translates to:
  /// **'Connecting the private audio channel...'**
  String get radioConnecting;

  /// No description provided for @radioDisabledBy.
  ///
  /// In en, this message translates to:
  /// **'{actor} disabled Radio mode'**
  String radioDisabledBy(Object actor);

  /// No description provided for @radioEnabledBy.
  ///
  /// In en, this message translates to:
  /// **'{actor} enabled Radio mode'**
  String radioEnabledBy(Object actor);

  /// No description provided for @radioMode.
  ///
  /// In en, this message translates to:
  /// **'Radio mode'**
  String get radioMode;

  /// No description provided for @radioModeDescription.
  ///
  /// In en, this message translates to:
  /// **'Short push-to-talk transmissions of up to 10 seconds. Radio becomes available only after both contacts enable it.'**
  String get radioModeDescription;

  /// No description provided for @radioReady.
  ///
  /// In en, this message translates to:
  /// **'Hold to talk'**
  String get radioReady;

  /// No description provided for @radioReceiving.
  ///
  /// In en, this message translates to:
  /// **'{name} is transmitting'**
  String radioReceiving(Object name);

  /// No description provided for @radioReconnecting.
  ///
  /// In en, this message translates to:
  /// **'Radio is reconnecting...'**
  String get radioReconnecting;

  /// No description provided for @radioRequestingFloor.
  ///
  /// In en, this message translates to:
  /// **'Requesting the channel...'**
  String get radioRequestingFloor;

  /// No description provided for @radioTransmitting.
  ///
  /// In en, this message translates to:
  /// **'Transmitting'**
  String get radioTransmitting;

  /// No description provided for @radioTransportFailure.
  ///
  /// In en, this message translates to:
  /// **'Radio: {code, select, endpoint_unavailable {endpoint unavailable} connect_timeout {connection timeout} stream_reset {stream reset} idle_timeout {idle timeout} network_changed {network changed} worker_unavailable {audio worker unavailable} protocol {protocol error} other {unknown transport error}}'**
  String radioTransportFailure(String code);

  /// No description provided for @radioUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Radio is temporarily unavailable'**
  String get radioUnavailable;

  /// No description provided for @radioWaitingForPeer.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the contact to enable Radio'**
  String get radioWaitingForPeer;

  /// No description provided for @rawDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Raw diagnostics'**
  String get rawDiagnostics;

  /// No description provided for @reactToMessage.
  ///
  /// In en, this message translates to:
  /// **'React'**
  String get reactToMessage;

  /// No description provided for @read.
  ///
  /// In en, this message translates to:
  /// **'Read'**
  String get read;

  /// No description provided for @receivingSecurely.
  ///
  /// In en, this message translates to:
  /// **'Receiving securely'**
  String get receivingSecurely;

  /// No description provided for @recentEmoji.
  ///
  /// In en, this message translates to:
  /// **'Recently used'**
  String get recentEmoji;

  /// No description provided for @recentInvitations.
  ///
  /// In en, this message translates to:
  /// **'Recent invitations'**
  String get recentInvitations;

  /// No description provided for @reconnectAttempts.
  ///
  /// In en, this message translates to:
  /// **'Reconnect attempts'**
  String get reconnectAttempts;

  /// No description provided for @reconnecting.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting'**
  String get reconnecting;

  /// No description provided for @reconnectingPeerThrough.
  ///
  /// In en, this message translates to:
  /// **'{provider, select, iroh {Reconnecting to peer through Iroh} memory {Reconnecting to peer through Memory test} other {Reconnecting to peer through {provider}}}'**
  String reconnectingPeerThrough(String provider);

  /// No description provided for @reconnectingShort.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting'**
  String get reconnectingShort;

  /// No description provided for @recordingTransfers.
  ///
  /// In en, this message translates to:
  /// **'Recordings'**
  String get recordingTransfers;

  /// No description provided for @redactedDeveloperEventStream.
  ///
  /// In en, this message translates to:
  /// **'Redacted developer event stream'**
  String get redactedDeveloperEventStream;

  /// No description provided for @redactedHealthEventsReadable.
  ///
  /// In en, this message translates to:
  /// **'Redacted health events readable'**
  String get redactedHealthEventsReadable;

  /// No description provided for @redactedSchedulerDescription.
  ///
  /// In en, this message translates to:
  /// **'Redacted scheduler explanation; contact identifiers are never shown here.'**
  String get redactedSchedulerDescription;

  /// No description provided for @reduceMotion.
  ///
  /// In en, this message translates to:
  /// **'Reduce motion'**
  String get reduceMotion;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @refreshProviderRoute.
  ///
  /// In en, this message translates to:
  /// **'Refresh provider route'**
  String get refreshProviderRoute;

  /// No description provided for @regressionScore.
  ///
  /// In en, this message translates to:
  /// **'Regression score'**
  String get regressionScore;

  /// No description provided for @reject.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get reject;

  /// No description provided for @remoteIdentity.
  ///
  /// In en, this message translates to:
  /// **'Identity {id}'**
  String remoteIdentity(String id);

  /// No description provided for @remoteIdentityTitle.
  ///
  /// In en, this message translates to:
  /// **'Remote identity'**
  String get remoteIdentityTitle;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// No description provided for @removeAttachment.
  ///
  /// In en, this message translates to:
  /// **'Remove attachment'**
  String get removeAttachment;

  /// No description provided for @removeBookmark.
  ///
  /// In en, this message translates to:
  /// **'Remove bookmark'**
  String get removeBookmark;

  /// No description provided for @removeContact.
  ///
  /// In en, this message translates to:
  /// **'Remove contact'**
  String get removeContact;

  /// No description provided for @removeContactDescription.
  ///
  /// In en, this message translates to:
  /// **'This removes the local relationship, conversation history, pending work and protected peer credential.'**
  String get removeContactDescription;

  /// No description provided for @removeContactTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}?'**
  String removeContactTitle(Object name);

  /// No description provided for @renameContact.
  ///
  /// In en, this message translates to:
  /// **'Rename contact'**
  String get renameContact;

  /// No description provided for @reply.
  ///
  /// In en, this message translates to:
  /// **'Reply'**
  String get reply;

  /// No description provided for @resetBaseline.
  ///
  /// In en, this message translates to:
  /// **'Reset baseline'**
  String get resetBaseline;

  /// No description provided for @resetVerification.
  ///
  /// In en, this message translates to:
  /// **'Reset verification'**
  String get resetVerification;

  /// No description provided for @restartApplication.
  ///
  /// In en, this message translates to:
  /// **'Restart application'**
  String get restartApplication;

  /// No description provided for @restoreConversation.
  ///
  /// In en, this message translates to:
  /// **'Restore conversation'**
  String get restoreConversation;

  /// No description provided for @retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;

  /// No description provided for @retryGeneration.
  ///
  /// In en, this message translates to:
  /// **'Retry generation'**
  String get retryGeneration;

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

  /// No description provided for @roundTrip.
  ///
  /// In en, this message translates to:
  /// **'Round trip'**
  String get roundTrip;

  /// No description provided for @route.
  ///
  /// In en, this message translates to:
  /// **'Provider route'**
  String get route;

  /// No description provided for @routeRefreshRequested.
  ///
  /// In en, this message translates to:
  /// **'Provider route refresh requested.'**
  String get routeRefreshRequested;

  /// No description provided for @routeRefreshRequired.
  ///
  /// In en, this message translates to:
  /// **'The communication route is being refreshed. Try again shortly.'**
  String get routeRefreshRequired;

  /// No description provided for @runSelfTest.
  ///
  /// In en, this message translates to:
  /// **'Run self-test'**
  String get runSelfTest;

  /// No description provided for @runtimeHealth.
  ///
  /// In en, this message translates to:
  /// **'Runtime health'**
  String get runtimeHealth;

  /// No description provided for @runtimeNotReadyDiagnostic.
  ///
  /// In en, this message translates to:
  /// **'{provider}'**
  String runtimeNotReadyDiagnostic(Object provider);

  /// No description provided for @runtimePreparationFailed.
  ///
  /// In en, this message translates to:
  /// **'Torca could not prepare the local encrypted runtime. Your identity has not been changed.'**
  String get runtimePreparationFailed;

  /// No description provided for @runtimeTab.
  ///
  /// In en, this message translates to:
  /// **'Runtime'**
  String get runtimeTab;

  /// No description provided for @runtimeUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The secure Torca runtime is currently unavailable.'**
  String get runtimeUnavailable;

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

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @saveAs.
  ///
  /// In en, this message translates to:
  /// **'Save as'**
  String get saveAs;

  /// No description provided for @saveAttachment.
  ///
  /// In en, this message translates to:
  /// **'Save attachment'**
  String get saveAttachment;

  /// No description provided for @saving.
  ///
  /// In en, this message translates to:
  /// **'Saving…'**
  String get saving;

  /// No description provided for @scanQr.
  ///
  /// In en, this message translates to:
  /// **'Scan QR'**
  String get scanQr;

  /// No description provided for @scheduledWork.
  ///
  /// In en, this message translates to:
  /// **'Scheduled work'**
  String get scheduledWork;

  /// No description provided for @searchChats.
  ///
  /// In en, this message translates to:
  /// **'Search chats'**
  String get searchChats;

  /// No description provided for @searchConversationHint.
  ///
  /// In en, this message translates to:
  /// **'Search this conversation'**
  String get searchConversationHint;

  /// No description provided for @searchMessages.
  ///
  /// In en, this message translates to:
  /// **'Search messages'**
  String get searchMessages;

  /// No description provided for @searchResultsCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1 {{count} result} other {{count} results}}'**
  String searchResultsCount(int count);

  /// No description provided for @secureRuntimeNotReady.
  ///
  /// In en, this message translates to:
  /// **'Secure runtime is not ready'**
  String get secureRuntimeNotReady;

  /// No description provided for @selectConversation.
  ///
  /// In en, this message translates to:
  /// **'Select a conversation'**
  String get selectConversation;

  /// No description provided for @sendMessage.
  ///
  /// In en, this message translates to:
  /// **'Send message'**
  String get sendMessage;

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

  /// No description provided for @senderContact.
  ///
  /// In en, this message translates to:
  /// **'Contact'**
  String get senderContact;

  /// No description provided for @senderYou.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get senderYou;

  /// No description provided for @sendingSecurely.
  ///
  /// In en, this message translates to:
  /// **'Sending securely'**
  String get sendingSecurely;

  /// No description provided for @sent.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get sent;

  /// No description provided for @sentAt.
  ///
  /// In en, this message translates to:
  /// **'Sent {time}'**
  String sentAt(Object time);

  /// No description provided for @deliveredAt.
  ///
  /// In en, this message translates to:
  /// **'Delivered {time}'**
  String deliveredAt(Object time);

  /// No description provided for @seenAt.
  ///
  /// In en, this message translates to:
  /// **'Seen at {time}'**
  String seenAt(Object time);

  /// No description provided for @receivedAt.
  ///
  /// In en, this message translates to:
  /// **'Received {time}'**
  String receivedAt(Object time);

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// No description provided for @sharedMedia.
  ///
  /// In en, this message translates to:
  /// **'Shared media and files'**
  String get sharedMedia;

  /// No description provided for @sharedMediaCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1 {1 item} other {{count} items}}'**
  String sharedMediaCount(int count);

  /// No description provided for @sourceCommit.
  ///
  /// In en, this message translates to:
  /// **'Source commit'**
  String get sourceCommit;

  /// No description provided for @startConversation.
  ///
  /// In en, this message translates to:
  /// **'Start conversation'**
  String get startConversation;

  /// No description provided for @startObservation.
  ///
  /// In en, this message translates to:
  /// **'Start observation'**
  String get startObservation;

  /// No description provided for @startingSecureNetwork.
  ///
  /// In en, this message translates to:
  /// **'Starting communication…'**
  String get startingSecureNetwork;

  /// No description provided for @startingShort.
  ///
  /// In en, this message translates to:
  /// **'Starting'**
  String get startingShort;

  /// No description provided for @state.
  ///
  /// In en, this message translates to:
  /// **'State'**
  String get state;

  /// No description provided for @staticIdle.
  ///
  /// In en, this message translates to:
  /// **'Static when idle'**
  String get staticIdle;

  /// No description provided for @status.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get status;

  /// No description provided for @stopObservation.
  ///
  /// In en, this message translates to:
  /// **'Stop observation'**
  String get stopObservation;

  /// No description provided for @storageEpoch.
  ///
  /// In en, this message translates to:
  /// **'Storage epoch'**
  String get storageEpoch;

  /// No description provided for @storageFailure.
  ///
  /// In en, this message translates to:
  /// **'Encrypted local storage could not complete the operation.'**
  String get storageFailure;

  /// No description provided for @system.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get system;

  /// No description provided for @systemDefaultAudioDevice.
  ///
  /// In en, this message translates to:
  /// **'System default device'**
  String get systemDefaultAudioDevice;

  /// No description provided for @systemLanguage.
  ///
  /// In en, this message translates to:
  /// **'System language'**
  String get systemLanguage;

  /// No description provided for @terminal.
  ///
  /// In en, this message translates to:
  /// **'Terminal'**
  String get terminal;

  /// No description provided for @today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get today;

  /// No description provided for @todayUpper.
  ///
  /// In en, this message translates to:
  /// **'TODAY'**
  String get todayUpper;

  /// No description provided for @transferFailed.
  ///
  /// In en, this message translates to:
  /// **'Transfer failed'**
  String get transferFailed;

  /// No description provided for @transfers.
  ///
  /// In en, this message translates to:
  /// **'Transfers'**
  String get transfers;

  /// No description provided for @transport.
  ///
  /// In en, this message translates to:
  /// **'Transport'**
  String get transport;

  /// No description provided for @typeToSearchConversation.
  ///
  /// In en, this message translates to:
  /// **'Type to search this conversation.'**
  String get typeToSearchConversation;

  /// No description provided for @unavailable.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get unavailable;

  /// No description provided for @unblockContact.
  ///
  /// In en, this message translates to:
  /// **'Unblock contact'**
  String get unblockContact;

  /// No description provided for @unknown.
  ///
  /// In en, this message translates to:
  /// **'Unknown'**
  String get unknown;

  /// No description provided for @unknownCountry.
  ///
  /// In en, this message translates to:
  /// **'Unknown'**
  String get unknownCountry;

  /// No description provided for @unmuteConversation.
  ///
  /// In en, this message translates to:
  /// **'Unmute conversation'**
  String get unmuteConversation;

  /// No description provided for @unpinConversation.
  ///
  /// In en, this message translates to:
  /// **'Unpin conversation'**
  String get unpinConversation;

  /// No description provided for @unverified.
  ///
  /// In en, this message translates to:
  /// **'Unverified'**
  String get unverified;

  /// No description provided for @variant.
  ///
  /// In en, this message translates to:
  /// **'Variant'**
  String get variant;

  /// No description provided for @verification.
  ///
  /// In en, this message translates to:
  /// **'Verification'**
  String get verification;

  /// No description provided for @verified.
  ///
  /// In en, this message translates to:
  /// **'Verified'**
  String get verified;

  /// No description provided for @verifiedOnDevice.
  ///
  /// In en, this message translates to:
  /// **'Verified on device'**
  String get verifiedOnDevice;

  /// No description provided for @verifyContact.
  ///
  /// In en, this message translates to:
  /// **'Verify contact'**
  String get verifyContact;

  /// No description provided for @verifyFingerprintBeforeAccepting.
  ///
  /// In en, this message translates to:
  /// **'A device joined this invitation. Verify the fingerprint before accepting the contact.'**
  String get verifyFingerprintBeforeAccepting;

  /// No description provided for @visualActivity.
  ///
  /// In en, this message translates to:
  /// **'Avatar and visual activity'**
  String get visualActivity;

  /// No description provided for @voiceClipRecording.
  ///
  /// In en, this message translates to:
  /// **'Recording voice clip, {secondsLeft} s remaining'**
  String voiceClipRecording(Object secondsLeft);

  /// No description provided for @voiceClipRecordingFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not record the voice clip.'**
  String get voiceClipRecordingFailed;

  /// No description provided for @voiceMessage.
  ///
  /// In en, this message translates to:
  /// **'Voice message'**
  String get voiceMessage;

  /// No description provided for @voiceMessagePlayed.
  ///
  /// In en, this message translates to:
  /// **'Played'**
  String get voiceMessagePlayed;

  /// No description provided for @voiceMessageReady.
  ///
  /// In en, this message translates to:
  /// **'Ready to play'**
  String get voiceMessageReady;

  /// No description provided for @waitingForDependency.
  ///
  /// In en, this message translates to:
  /// **'Waiting for: {dependency}'**
  String waitingForDependency(Object dependency);

  /// No description provided for @waitingForPeer.
  ///
  /// In en, this message translates to:
  /// **'Waiting for peer'**
  String get waitingForPeer;

  /// No description provided for @waitingToReceive.
  ///
  /// In en, this message translates to:
  /// **'Waiting to receive'**
  String get waitingToReceive;

  /// No description provided for @wakeSources.
  ///
  /// In en, this message translates to:
  /// **'Wake sources'**
  String get wakeSources;

  /// No description provided for @whyAwake.
  ///
  /// In en, this message translates to:
  /// **'Why awake'**
  String get whyAwake;

  /// No description provided for @yesterday.
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get yesterday;

  /// No description provided for @yourIdentity.
  ///
  /// In en, this message translates to:
  /// **'Your identity'**
  String get yourIdentity;

  /// No description provided for @yourInvitation.
  ///
  /// In en, this message translates to:
  /// **'Your invitation'**
  String get yourInvitation;

  /// No description provided for @zeroDelayDeadlines.
  ///
  /// In en, this message translates to:
  /// **'Zero-delay deadlines'**
  String get zeroDelayDeadlines;
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

import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
import 'runtime_network_status.dart';

class ConversationHeaderSurface extends StatelessWidget {
  const ConversationHeaderSurface({
    required this.child,
    this.radioActive = false,
    this.topSafeArea = false,
    this.padding = const EdgeInsets.fromLTRB(16, 10, 8, 10),
    super.key,
  });

  final Widget child;
  final bool radioActive;
  final bool topSafeArea;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final surface = colors.surface.withValues(alpha: 0.90);
    final fill = radioActive
        ? Color.alphaBlend(colors.error.withValues(alpha: 0.12), surface)
        : surface;
    return ClipRect(
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 14, sigmaY: 14),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: fill,
            border: Border(
              bottom: BorderSide(
                color: colors.outlineVariant.withValues(alpha: 0.55),
              ),
            ),
          ),
          child: SafeArea(
            top: topSafeArea,
            bottom: false,
            left: topSafeArea,
            right: topSafeArea,
            minimum: EdgeInsets.zero,
            child: ConstrainedBox(
              constraints: const BoxConstraints(minHeight: 60),
              child: Padding(padding: padding, child: child),
            ),
          ),
        ),
      ),
    );
  }
}

class ConversationHeader extends StatelessWidget {
  const ConversationHeader({
    required this.contact,
    required this.onConnectionDetails,
    this.gateway,
    this.snapshot,
    this.radio,
    this.session,
    this.sending = false,
    this.receiving = false,
    this.compact = false,
    this.leading,
    this.instantContact = false,
    this.instantContactBusy = false,
    this.radioSupported = true,
    this.onInstantContactChanged,
    this.onConversationActions,
    super.key,
  });

  final ContactDto? contact;
  final VoidCallback onConnectionDetails;
  final bool compact;
  final EngineGateway? gateway;

  final AppSnapshotDto? snapshot;
  final RadioContactDto? radio;
  final RadioSessionDto? session;
  final bool sending;
  final bool receiving;
  final Widget? leading;
  final bool instantContact;
  final bool instantContactBusy;
  final ValueChanged<bool>? onInstantContactChanged;
  final VoidCallback? onConversationActions;
  final bool radioSupported;

  @override
  Widget build(BuildContext context) {
    final value = contact;
    final blocked = value?.typedStatus == ContactStatus.blocked;
    final name = value?.displayName ?? context.strings.contactLabel;
    final radioState = session?.typedState ?? radio?.typedState;
    final avatar = TorcaDeviceAvatar(
      label: name,
      identityId: value?.remoteIdentityId,
      fallbackIdentityId: value?.id,
      size: compact ? 32 : 40,
      presentation: AvatarActivityPresentation.resolve(
        blocked: blocked,
        talking: radioState == RadioState.receiving,
        listening: radioState == RadioState.transmitting,
        sending: sending,
        receiving: receiving,
        waking: radioState == RadioState.connecting,
        online: value?.typedAvailability == PeerAvailability.reachable,
        error: radioState == RadioState.reconnecting,
      ),
    );
    final radioAction =
        radioSupported && value != null && gateway != null && radio != null
        ? _RadioHeaderAction(
            gateway: gateway!,
            contact: value,
            radio: radio!,
            session: session,
            compact: compact,
          )
        : null;
    final networkStatus = snapshot == null
        ? null
        : RuntimeNetworkStatus(snapshot: snapshot!, compact: compact);

    if (compact) {
      return LayoutBuilder(
        builder: (context, constraints) {
          final actionRow = Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              _InstantContactAction(
                enabled: instantContact,
                busy: instantContactBusy,
                compact: true,
                onChanged: onInstantContactChanged,
              ),
              if (radioAction != null) radioAction,
              IconButton(
                tooltip: context.strings.connectionDetails,
                onPressed: onConnectionDetails,
                icon: Icon(context.torcaIcons.info),
              ),
              if (onConversationActions != null)
                IconButton(
                  tooltip: context.strings.conversationActions,
                  onPressed: onConversationActions,
                  icon: Icon(context.torcaIcons.more),
                ),
            ],
          );
          final identityRow = Row(
            children: <Widget>[
              if (leading != null) ...<Widget>[
                leading!,
                const SizedBox(width: 2),
              ],
              avatar,
              const SizedBox(width: 10),
              Expanded(child: _contactText(context, value, name, blocked)),
            ],
          );
          final narrow = constraints.maxWidth < 430;
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (narrow) ...<Widget>[
                Row(children: <Widget>[Expanded(child: identityRow)]),
                Align(alignment: Alignment.centerRight, child: actionRow),
              ] else
                Row(
                  children: <Widget>[
                    Expanded(child: identityRow),
                    actionRow,
                  ],
                ),
              if (networkStatus != null) ...<Widget>[
                const SizedBox(height: 2),
                Align(alignment: Alignment.centerRight, child: networkStatus),
              ],
            ],
          );
        },
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        // The conversation pane can be narrower than the physical window on
        // desktop. Keep the network monitor readable, but move it below the
        // identity/actions row before the switches start competing with the
        // contact name for horizontal space.
        final narrowPane = constraints.maxWidth < 700;
        final paneNetworkStatus = snapshot == null
            ? null
            : RuntimeNetworkStatus(snapshot: snapshot!, compact: narrowPane);
        final identityRow = Row(
          children: <Widget>[
            if (leading != null) ...<Widget>[
              leading!,
              const SizedBox(width: 2),
            ],
            avatar,
            const SizedBox(width: 10),
            Expanded(child: _contactText(context, value, name, blocked)),
            _InstantContactAction(
              enabled: instantContact,
              busy: instantContactBusy,
              compact: narrowPane,
              onChanged: onInstantContactChanged,
            ),
            if (radioAction != null) radioAction,
            IconButton(
              tooltip: context.strings.connectionDetails,
              onPressed: onConnectionDetails,
              icon: Icon(context.torcaIcons.info),
            ),
            if (onConversationActions != null)
              IconButton(
                tooltip: context.strings.conversationActions,
                onPressed: onConversationActions,
                icon: Icon(context.torcaIcons.more),
              ),
          ],
        );
        if (paneNetworkStatus == null || !narrowPane) {
          return Row(
            children: <Widget>[
              if (leading != null) ...<Widget>[
                leading!,
                const SizedBox(width: 2),
              ],
              avatar,
              const SizedBox(width: 10),
              Expanded(child: _contactText(context, value, name, blocked)),
              if (paneNetworkStatus != null) ...<Widget>[
                const SizedBox(width: 8),
                paneNetworkStatus,
              ],
              _InstantContactAction(
                enabled: instantContact,
                busy: instantContactBusy,
                compact: false,
                onChanged: onInstantContactChanged,
              ),
              if (radioAction != null) radioAction,
              IconButton(
                tooltip: context.strings.connectionDetails,
                onPressed: onConnectionDetails,
                icon: Icon(context.torcaIcons.info),
              ),
              if (onConversationActions != null)
                IconButton(
                  tooltip: context.strings.conversationActions,
                  onPressed: onConversationActions,
                  icon: Icon(context.torcaIcons.more),
                ),
            ],
          );
        }
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            identityRow,
            Align(alignment: Alignment.centerRight, child: paneNetworkStatus),
          ],
        );
      },
    );
  }

  Widget _contactText(
    BuildContext context,
    ContactDto? value,
    String name,
    bool blocked,
  ) => Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.start,
    children: <Widget>[
      Text(
        name,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: compact
            ? Theme.of(context).textTheme.titleSmall
            : Theme.of(context).textTheme.titleMedium,
      ),
      if (value != null)
        blocked
            ? Text(
                context.strings.blocked,
                style: Theme.of(context).textTheme.bodySmall,
              )
            : InkWell(
                onTap: onConnectionDetails,
                child: Text(
                  _presenceLabel(context, value),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: value.typedPresenceState == PresenceState.online
                        ? Theme.of(context).colorScheme.primary
                        : null,
                  ),
                ),
              ),
    ],
  );
}

class _InstantContactAction extends StatelessWidget {
  const _InstantContactAction({
    required this.enabled,
    required this.busy,
    required this.compact,
    required this.onChanged,
  });

  final bool enabled;
  final bool busy;
  final bool compact;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    // A header action without a callback is only a presentation preview. Do
    // not render a misleading, non-functional control in that case (this also
    // keeps the compact header honest for read-only embeds).
    if (onChanged == null) return const SizedBox.shrink();
    final label = enabled
        ? context.strings.instantModeEnabled
        : context.strings.instantMode;
    return Semantics(
      label: label,
      toggled: enabled,
      child: Tooltip(
        message: label,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(
              context.torcaIcons.instant,
              size: compact ? 18 : 20,
              color: enabled
                  ? Theme.of(context).colorScheme.primary
                  : Theme.of(context).colorScheme.onSurfaceVariant,
            ),
            TorcaSwitch(
              value: enabled,
              semanticLabel: label,
              onChanged: busy ? null : onChanged,
            ),
          ],
        ),
      ),
    );
  }
}

class _RadioHeaderAction extends StatelessWidget {
  const _RadioHeaderAction({
    required this.gateway,
    required this.contact,
    required this.radio,
    this.session,
    required this.compact,
  });

  final EngineGateway gateway;
  final ContactDto contact;
  final RadioContactDto radio;
  final RadioSessionDto? session;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final state = session?.typedState ?? radio.typedState;
    final capturing = state == RadioState.transmitting;
    final receiving = state == RadioState.receiving;
    final colors = Theme.of(context).colorScheme;
    return Semantics(
      label: capturing
          ? context.strings.radioTransmitting
          : receiving
          ? context.strings.radioReceiving(contact.displayName)
          : context.strings.radioMode,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (capturing)
            Padding(
              padding: const EdgeInsets.only(right: 4),
              child: Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  color: colors.error,
                  borderRadius: context.torcaTokens.terminal
                      ? BorderRadius.zero
                      : BorderRadius.circular(context.torcaTokens.radiusSmall),
                ),
              ),
            ),
          Icon(
            context.torcaIcons.radio,
            size: compact ? 18 : 20,
            color: capturing || receiving
                ? colors.error
                : colors.onSurfaceVariant,
          ),
          TorcaSwitch(
            value: radio.localEnabled,
            semanticLabel: context.strings.radioMode,
            onChanged: (enabled) => _setEnabled(context, enabled),
          ),
        ],
      ),
    );
  }

  Future<void> _setEnabled(BuildContext context, bool enabled) async {
    final result = await gateway.execute(
      SetRadioEnabledCommandDto(contactIdHex: contact.id, enabled: enabled),
    );
    if (!context.mounted || result.ok) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(context.strings.couldNotStartRadio)));
  }
}

String _presenceLabel(BuildContext context, ContactDto contact) {
  if (contact.typedPresenceState == PresenceState.online) return 'online';
  final milliseconds = contact.lastSeenAtMs;
  if (milliseconds == null || milliseconds <= 0) {
    return contact.peerHealth.typedState == TransportState.reconnecting
        ? 'reconnecting'
        : 'offline';
  }
  final value = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
  final now = DateTime.now();
  final time = MaterialLocalizations.of(
    context,
  ).formatTimeOfDay(TimeOfDay.fromDateTime(value));
  if (value.year == now.year &&
      value.month == now.month &&
      value.day == now.day) {
    return 'last seen today at $time';
  }
  final yesterday = now.subtract(const Duration(days: 1));
  if (value.year == yesterday.year &&
      value.month == yesterday.month &&
      value.day == yesterday.day) {
    return 'last seen yesterday at $time';
  }
  return 'last seen ${MaterialLocalizations.of(context).formatMediumDate(value)}';
}

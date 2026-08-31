import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import 'bridge_error_presenter.dart';

/// Shared contact mutations and confirmations used by list and detail views.
/// Returning `true` means the command completed successfully.
class ContactActions {
  const ContactActions._();

  static Future<bool> rename(
    BuildContext context,
    EngineGateway gateway,
    ContactDto contact,
  ) async {
    final strings = context.l10n;
    var draft = contact.displayName;
    final value = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(strings.renameContact),
        content: TextFormField(
          initialValue: contact.displayName,
          autofocus: true,
          maxLength: 64,
          decoration: InputDecoration(labelText: strings.localName),
          onChanged: (value) => draft = value,
          onFieldSubmitted: (value) => Navigator.of(dialogContext).pop(value),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: Text(strings.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(draft),
            child: Text(strings.save),
          ),
        ],
      ),
    );
    final name = value?.trim();
    if (name == null || name.isEmpty || !context.mounted) return false;
    return _execute(
      context,
      gateway,
      RenameContactCommandDto(contactIdHex: contact.id, displayName: name),
      strings.couldNotRenameContact,
    );
  }

  static Future<bool> toggleBlock(
    BuildContext context,
    EngineGateway gateway,
    ContactDto contact,
  ) async {
    final strings = context.l10n;
    final blocking = contact.typedStatus != ContactStatus.blocked;
    if (blocking &&
        !await _confirm(
          context,
          strings.blockContactTitle(contact.displayName),
          strings.blockContactDescription,
          strings.blockContact,
        )) {
      return false;
    }
    if (!context.mounted) return false;
    return _execute(
      context,
      gateway,
      blocking
          ? BlockContactCommandDto(contactIdHex: contact.id)
          : UnblockContactCommandDto(contactIdHex: contact.id),
      blocking ? strings.couldNotBlockContact : strings.couldNotUnblockContact,
    );
  }

  static Future<bool> remove(
    BuildContext context,
    EngineGateway gateway,
    ContactDto contact,
  ) async {
    final strings = context.l10n;
    if (!await _confirm(
      context,
      strings.removeContactTitle(contact.displayName),
      strings.removeContactDescription,
      strings.remove,
    )) {
      return false;
    }
    if (!context.mounted) return false;
    return _execute(
      context,
      gateway,
      RemoveContactCommandDto(contactIdHex: contact.id),
      strings.couldNotRemoveContact,
    );
  }

  static Future<bool> _confirm(
    BuildContext context,
    String title,
    String message,
    String action,
  ) async =>
      await showDialog<bool>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text(title),
          content: Text(message),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(false),
              child: Text(context.l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(dialogContext).pop(true),
              child: Text(action),
            ),
          ],
        ),
      ) ??
      false;

  static Future<bool> _execute(
    BuildContext context,
    EngineGateway gateway,
    BridgeCommandDto command,
    String fallback,
  ) async {
    BridgeResultDto result;
    try {
      result = await gateway.execute(command);
    } on Object catch (_) {
      // A native assertion or a temporarily unavailable runtime must not
      // leave the modal route mounted with an unhandled Future. Keep the
      // contact screen usable and let the user retry after diagnostics.
      if (context.mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(fallback)));
      }
      return false;
    }
    if (!context.mounted) return result.ok;
    if (!result.ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            BridgeErrorPresenter.localized(context, result, fallback: fallback),
          ),
        ),
      );
    }
    return result.ok;
  }
}

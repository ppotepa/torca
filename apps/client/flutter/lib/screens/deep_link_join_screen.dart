import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../widgets/runtime_network_status.dart';
import 'pairing_screen.dart';

/// @deprecated Deep links are routed through `showJoinInvitationModal` so
/// Contacts, QR and platform links share one join flow. Kept as a source
/// compatibility shim for downstream integrations that still import this
/// symbol; do not add new navigation to it.
@Deprecated('Use showJoinInvitationModal from pairing_screen.dart')
class DeepLinkJoinScreen extends StatelessWidget {
  const DeepLinkJoinScreen({
    required this.gateway,
    required this.code,
    super.key,
  });
  final EngineGateway gateway;
  final String code;
  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: const RuntimeAppBar(title: Text('Join Torca invitation')),
    body: Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Icon(context.torcaIcons.link, size: 48),
              const SizedBox(height: 16),
              const Text('Invitation code', textAlign: TextAlign.center),
              const SizedBox(height: 8),
              SelectableText(
                code,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.headlineSmall,
              ),
              const SizedBox(height: 20),
              FilledButton(
                onPressed: () => showJoinInvitationModal(
                  context,
                  gateway,
                  initialCode: code,
                ),
                child: const Text('Join invitation'),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

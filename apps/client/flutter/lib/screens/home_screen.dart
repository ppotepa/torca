import 'package:flutter/material.dart';
import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({required this.gateway, super.key});
  final EngineGateway gateway;
  @override Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(valueListenable: gateway.snapshots, builder: (context, snapshot, _) => Scaffold(appBar: AppBar(title: Text(snapshot.identity?.displayName ?? 'Torca'), actions: [IconButton(icon: const Icon(Icons.monitor_heart_outlined), onPressed: () => Navigator.of(context).push(MaterialPageRoute(builder: (_) => const DiagnosticsScreen()))) ]), body: snapshot.identity == null ? _IdentitySetup(gateway: gateway) : ListView(children: [for (final conversation in snapshot.conversations) ListTile(title: Text('Contact ${conversation.contactId.substring(0, 8)}'), subtitle: Text(conversation.status), onTap: () => Navigator.of(context).push(MaterialPageRoute(builder: (_) => ConversationScreen(gateway: gateway, conversation: conversation))))]), floatingActionButton: snapshot.identity == null ? null : FloatingActionButton(onPressed: () => Navigator.of(context).push(MaterialPageRoute(builder: (_) => PairingScreen(gateway: gateway))), child: const Icon(Icons.person_add_alt_1))));
}

class _IdentitySetup extends StatefulWidget { const _IdentitySetup({required this.gateway}); final EngineGateway gateway; @override State<_IdentitySetup> createState() => _IdentitySetupState(); }
class _IdentitySetupState extends State<_IdentitySetup> { final controller = TextEditingController(); @override void dispose() { controller.dispose(); super.dispose(); } @override Widget build(BuildContext context) => Center(child: ConstrainedBox(constraints: const BoxConstraints(maxWidth: 360), child: Column(mainAxisSize: MainAxisSize.min, children: [TextField(controller: controller, decoration: const InputDecoration(labelText: 'Display name')), const SizedBox(height: 12), FilledButton(onPressed: () => widget.gateway.execute(CreateIdentityCommandDto(identityIdHex: DateTime.now().microsecondsSinceEpoch.toRadixString(16).padLeft(32, '0').substring(0, 32), displayName: controller.text, atMs: DateTime.now().millisecondsSinceEpoch)), child: const Text('Create local identity'))]))); }

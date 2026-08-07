import 'package:flutter/foundation.dart';

class AppNavigationController {
  final ValueNotifier<String?> conversationRequest = ValueNotifier<String?>(null);
  final ValueNotifier<String?> pairingCodeRequest = ValueNotifier<String?>(null);
  final ValueNotifier<int> newPairingRequest = ValueNotifier<int>(0);

  void openConversation(String conversationId) {
    conversationRequest.value = conversationId;
  }

  void openPairing(String code) {
    pairingCodeRequest.value = code;
  }

  void requestNewPairing() {
    newPairingRequest.value = newPairingRequest.value + 1;
  }

  void clearConversationRequest() {
    conversationRequest.value = null;
  }

  void clearPairingRequest() {
    pairingCodeRequest.value = null;
  }

  void dispose() {
    conversationRequest.dispose();
    pairingCodeRequest.dispose();
    newPairingRequest.dispose();
  }
}

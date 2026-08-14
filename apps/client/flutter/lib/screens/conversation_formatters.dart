part of 'conversation_screen.dart';

String safeExtension(String name) {
  final dot = name.lastIndexOf('.');
  if (dot < 0 || dot == name.length - 1) return '';
  final value = name.substring(dot);
  return RegExp(r'^\.[A-Za-z0-9]{1,10}$').hasMatch(value)
      ? value.toLowerCase()
      : '';
}

String? contentExtension(String mediaType) => switch (mediaType) {
  'image/jpeg' => '.jpg',
  'image/png' => '.png',
  'image/gif' => '.gif',
  'image/webp' => '.webp',
  'video/mp4' => '.mp4',
  'video/webm' => '.webm',
  'audio/mpeg' => '.mp3',
  'audio/ogg' => '.ogg',
  'audio/wav' => '.wav',
  'application/pdf' => '.pdf',
  _ => null,
};

bool hasVisualAttachmentPreview(String mediaType) =>
    mediaType.startsWith('image/') || mediaType.startsWith('video/');

String messageStatusLabel(String status, TorcaStrings strings) =>
    switch (status) {
      'queued' => strings.messageQueued,
      'sending' => strings.sendingSecurely,
      'sent' => strings.sent,
      'delivered' => strings.delivered,
      'read' => strings.read,
      'failed' => strings.deliveryFailed,
      'cancelled' => strings.cancelled,
      _ => status,
    };

bool sameDay(MessageDto first, MessageDto second) {
  final a = DateTime.fromMillisecondsSinceEpoch(first.createdAtMs).toLocal();
  final b = DateTime.fromMillisecondsSinceEpoch(second.createdAtMs).toLocal();
  return a.year == b.year && a.month == b.month && a.day == b.day;
}

ContactDto? contactForSnapshot(
  AppSnapshotDto snapshot,
  ConversationDto conversation,
) {
  for (final contact in snapshot.contacts) {
    if (contact.id == conversation.contactId) return contact;
  }
  return null;
}

import 'package:flutter/material.dart';
import 'package:iconsx_plus/iconsx_plus.dart';

@immutable
class TorcaIconSet extends ThemeExtension<TorcaIconSet> {
  const TorcaIconSet({
    required this.chats,
    required this.contacts,
    required this.invitations,
    required this.addContact,
    required this.contactInfo,
    required this.send,
    required this.attachment,
    required this.settings,
    required this.search,
    required this.close,
    required this.confirm,
  });

  final IconData chats;
  final IconData contacts;
  final IconData invitations;
  final IconData addContact;
  final IconData contactInfo;
  final IconData send;
  final IconData attachment;
  final IconData settings;
  final IconData search;
  final IconData close;
  final IconData confirm;

  static const modern = TorcaIconSet(
    chats: HeroIcons.chat_bubble_left_right,
    contacts: HeroIcons.user_group,
    invitations: HeroIcons.qr_code,
    addContact: HeroIcons.user_plus,
    contactInfo: HeroIcons.information_circle,
    send: HeroIcons.paper_airplane,
    attachment: HeroIcons.paper_clip,
    settings: HeroIcons.cog_6_tooth,
    search: HeroIcons.magnifying_glass,
    close: HeroIcons.x_mark,
    confirm: HeroIcons.check,
  );

  static const terminal = TorcaIconSet(
    chats: PixelArtIcons.message_text,
    contacts: PixelArtIcons.users,
    invitations: PixelArtIcons.camera,
    addContact: PixelArtIcons.user_plus,
    contactInfo: PixelArtIcons.info_box,
    send: PixelArtIcons.message_arrow_right,
    attachment: PixelArtIcons.attachment,
    settings: PixelArtIcons.sliders,
    search: PixelArtIcons.search,
    close: PixelArtIcons.close,
    confirm: PixelArtIcons.check,
  );

  @override
  TorcaIconSet copyWith({
    IconData? chats,
    IconData? contacts,
    IconData? invitations,
    IconData? addContact,
    IconData? contactInfo,
    IconData? send,
    IconData? attachment,
    IconData? settings,
    IconData? search,
    IconData? close,
    IconData? confirm,
  }) => TorcaIconSet(
    chats: chats ?? this.chats,
    contacts: contacts ?? this.contacts,
    invitations: invitations ?? this.invitations,
    addContact: addContact ?? this.addContact,
    contactInfo: contactInfo ?? this.contactInfo,
    send: send ?? this.send,
    attachment: attachment ?? this.attachment,
    settings: settings ?? this.settings,
    search: search ?? this.search,
    close: close ?? this.close,
    confirm: confirm ?? this.confirm,
  );

  @override
  TorcaIconSet lerp(covariant TorcaIconSet? other, double t) =>
      other == null || t < .5 ? this : other;
}

extension TorcaIconContext on BuildContext {
  TorcaIconSet get torcaIcons => Theme.of(this).extension<TorcaIconSet>()!;
}

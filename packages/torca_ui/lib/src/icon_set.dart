import 'package:flutter/material.dart';
import 'package:iconsx_plus/iconsx_plus.dart';

@immutable
class TorcaIconSet extends ThemeExtension<TorcaIconSet> {
  const TorcaIconSet({
    required this.pixel,
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

  final bool pixel;

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

  IconData get back =>
      pixel ? PixelArtIcons.chevron_left : HeroIcons.chevron_left;
  IconData get expand =>
      pixel ? PixelArtIcons.chevron_down : HeroIcons.chevron_down;
  IconData get collapse =>
      pixel ? PixelArtIcons.chevron_up : HeroIcons.chevron_up;
  IconData get more =>
      pixel ? PixelArtIcons.more_horizontal : HeroIcons.ellipsis_horizontal;
  IconData get reply =>
      pixel ? PixelArtIcons.reply : HeroIcons.arrow_uturn_left;
  IconData get forward =>
      pixel ? PixelArtIcons.reply : HeroIcons.arrow_uturn_right;
  IconData get copy => pixel ? PixelArtIcons.copy : HeroIcons.square_2_stack;
  IconData get retry => pixel ? PixelArtIcons.reload : HeroIcons.arrow_path;
  IconData get download =>
      pixel ? PixelArtIcons.download : HeroIcons.arrow_down_tray;
  IconData get jumpToLatest =>
      pixel ? PixelArtIcons.arrow_down : HeroIcons.arrow_down;
  IconData get remove => pixel ? PixelArtIcons.trash : HeroIcons.trash;
  IconData get edit => pixel ? PixelArtIcons.edit : HeroIcons.pencil_square;
  IconData get block => pixel ? PixelArtIcons.lock : HeroIcons.no_symbol;
  IconData get success => pixel ? PixelArtIcons.check : HeroIcons.check_circle;
  IconData get warning =>
      pixel ? PixelArtIcons.warning_box : HeroIcons.exclamation_triangle;
  IconData get error =>
      pixel ? PixelArtIcons.alert : HeroIcons.exclamation_circle;
  IconData get online => pixel ? PixelArtIcons.radio_signal : HeroIcons.signal;
  IconData get instant => pixel ? PixelArtIcons.zap : HeroIcons.bolt;
  IconData get radio => pixel ? PixelArtIcons.radio_handheld : HeroIcons.radio;
  IconData get pushToTalk =>
      pixel ? PixelArtIcons.radio_on : HeroIcons.microphone;
  IconData get play => pixel ? PixelArtIcons.play : HeroIcons.play;
  IconData get pause => pixel ? PixelArtIcons.pause : HeroIcons.pause;
  IconData get file => pixel ? PixelArtIcons.file : HeroIcons.document;
  IconData get image => pixel ? PixelArtIcons.image : HeroIcons.photo;
  IconData get video => pixel ? PixelArtIcons.video : HeroIcons.film;
  IconData get audio => pixel ? PixelArtIcons.music : HeroIcons.musical_note;
  IconData get pdf => pixel ? PixelArtIcons.book_open : HeroIcons.document_text;
  IconData get document => pixel ? PixelArtIcons.file_alt : HeroIcons.document;
  IconData get archive => pixel ? PixelArtIcons.archive : HeroIcons.archive_box;
  IconData get pin => pixel ? PixelArtIcons.pin : HeroIcons.map_pin;
  IconData get bookmark => pixel ? PixelArtIcons.bookmark : HeroIcons.bookmark;
  IconData get textFile => pixel ? PixelArtIcons.notes : HeroIcons.code_bracket;
  IconData get info => contactInfo;
  IconData get identity =>
      pixel ? PixelArtIcons.shield : HeroIcons.shield_check;
  IconData get diagnostics => pixel ? PixelArtIcons.chart : HeroIcons.chart_bar;
  IconData get notifications =>
      pixel ? PixelArtIcons.notification : HeroIcons.bell;
  IconData get appearance =>
      pixel ? PixelArtIcons.paint_bucket : HeroIcons.swatch;
  IconData get language =>
      pixel ? PixelArtIcons.message_text : HeroIcons.language;
  IconData get emoji =>
      pixel ? PixelArtIcons.message_text : HeroIcons.face_smile;
  IconData get open =>
      pixel ? PixelArtIcons.open : HeroIcons.arrow_top_right_on_square;
  IconData get save => pixel ? PixelArtIcons.save : HeroIcons.arrow_down_tray;
  IconData get scan =>
      pixel ? PixelArtIcons.camera : HeroIcons.viewfinder_circle;
  IconData get link => pixel ? PixelArtIcons.link : HeroIcons.link;
  IconData get reconnect => retry;
  IconData get queued => pixel ? PixelArtIcons.clock : HeroIcons.clock;
  IconData get sending => pixel ? PixelArtIcons.sync : HeroIcons.arrow_path;
  IconData get sent => pixel ? PixelArtIcons.check : HeroIcons.check;
  IconData get delivered =>
      pixel ? PixelArtIcons.check_double : HeroIcons.check_badge;
  IconData get read => pixel ? PixelArtIcons.eye : HeroIcons.eye;
  IconData get cancelled =>
      pixel ? PixelArtIcons.close_box : HeroIcons.x_circle;

  static const modern = TorcaIconSet(
    pixel: false,
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
    pixel: true,
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
    bool? pixel,
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
    pixel: pixel ?? this.pixel,
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
  TorcaIconSet get torcaIcons =>
      Theme.of(this).extension<TorcaIconSet>() ?? TorcaIconSet.modern;
}

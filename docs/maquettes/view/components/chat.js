(function (Torca) {
  'use strict';
  const C=Torca.components; const e=Torca.util.escape;
  C.conversationTile = function conversationTile(store, conversation, selected) {
    const contact=store.contact(conversation.contactId); if(!contact) return '';
    const preview=conversation.draft ? Torca.i18n.t('draft') : `${conversation.direction==='out' ? `${Torca.i18n.t('you')}: ` : ''}${conversation.lastMessage || ''}`;
    const stateIcons=[conversation.pinned?C.icon('pin','sm'):'',conversation.muted?C.icon('muted','sm'):''].join('');
    return `<button class="conversation-tile ${selected?'active':''}" type="button" data-conversation="${e(conversation.id)}">${C.avatar(contact)}<span class="conversation-tile__body"><span class="conversation-tile__top"><span class="conversation-tile__name">${e(contact.name)}</span><span class="conversation-tile__time">${e(Torca.util.formatRelative(conversation.lastAt))}</span></span><span class="conversation-tile__bottom"><span class="conversation-tile__preview ${conversation.draft?'draft':''}">${e(preview)}</span>${conversation.unread?C.badge(conversation.unread):''}</span></span><span class="conversation-tile__state">${stateIcons}</span></button>`;
  };
  function statusIcon(status) {
    if(status==='failed') return C.icon('warning','sm');
    if(status==='sending'||status==='queued') return C.icon('clock','sm');
    if(status==='read'||status==='delivered') return C.icon('doubleCheck','sm');
    return C.icon('check','sm');
  }
  C.messageBubble = function messageBubble(store, message, previous) {
    const grouped=previous && previous.direction===message.direction && message.createdAt-previous.createdAt < 120000;
    const reply=message.replyTo ? store.state.messages.find((m)=>m.id===message.replyTo) : null;
    let attachment='';
    if(message.attachment) {
      const a=message.attachment; const ico=a.kind==='video'?'video':a.kind==='image'?'image':'file';
      attachment=`<div class="attachment-card"><div class="attachment-card__icon">${C.icon(ico)}</div><div><strong>${e(a.name)}</strong><small>${e(a.size||'')}</small>${a.progress<100?`<div class="progress"><span style="width:${Number(a.progress)||0}%"></span></div>`:''}</div>${a.progress===100?C.icon('check','sm'):`<small>${Number(a.progress)||0}%</small>`}</div>`;
    }
    const reactions=message.reactions&&message.reactions.length?`<div class="reactions">${message.reactions.map((r)=>`<span class="reaction">${e(r)}</span>`).join('')}</div>`:'';
    const failure=message.status==='failed'?`<div class="message-bubble__failure">${C.icon('warning','sm')}<span>${e(Torca.i18n.t('failed'))}</span><button type="button" class="button outline" style="min-height:28px;padding:0 8px" data-retry-message="${e(message.id)}">${e(Torca.i18n.t('retry'))}</button></div>`:'';
    return `<div class="message-row ${message.direction} ${grouped?'grouped':''}" data-message="${e(message.id)}"><div class="message-bubble ${message.status==='failed'?'failed':''}">${reply?`<div class="message-bubble__reply">${e(reply.body||reply.attachment?.name||'Message')}</div>`:''}${message.body?`<div class="message-bubble__text">${e(message.body)}</div>`:''}${attachment}${reactions}<div class="message-bubble__footer"><span>${e(Torca.util.formatTime(message.createdAt))}</span>${message.direction==='out'?statusIcon(message.status):''}</div>${failure}</div></div>`;
  };
  C.composer = function composer(options) {
    const t=Torca.i18n.t.bind(Torca.i18n); const disabled=options.disabled?'disabled':'';
    return `<div class="composer">${options.reply?`<div class="composer__reply">${C.icon('back','sm')}<span style="flex:1">${e(options.reply.body || options.reply.attachment?.name || '')}</span>${C.iconButton('close',t('close'),'data-cancel-reply')}</div>`:''}<div class="composer__row"><button class="icon-button" type="button" data-attach title="${e(t('attach'))}" ${disabled}>${C.icon('plus')}</button><textarea class="composer__field" rows="1" data-message-input placeholder="${e(t('messagePlaceholder'))}" ${disabled}></textarea><button class="composer__action" type="button" data-send title="${e(t('send'))}" ${disabled}>${C.icon('mic')}</button></div></div>`;
  };
}(window.Torca));

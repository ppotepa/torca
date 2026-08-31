(function (Torca) {
  'use strict';
  class ChatsScreen extends Torca.core.Screen {
    render() {
      const t=Torca.i18n.t.bind(Torca.i18n); const store=this.app.store; const conversations=store.state.conversations;
      const list=conversations.length?`<div class="conversation-list">${conversations.map((c)=>Torca.components.conversationTile(store,c,false)).join('')}</div>`:Torca.components.emptyState('chats',t('noChats'),t('noChatsBody'),Torca.components.button(t('pairContact'),'plus','data-route="/invitations"','primary'));
      const placeholder=Torca.components.emptyState('chats',t('chats'),'Wybierz rozmowę z listy. Szczegóły techniczne połączenia pozostają poza głównym flow.');
      const body=this.app.isCompact()?list:`<div class="split-view"><aside class="list-pane">${list}</aside><section class="content-pane">${placeholder}</section></div>`;
      const actions=`${Torca.components.iconButton('search',t('search'),'data-chat-search')}${Torca.components.iconButton('plus',t('newMessage'),'data-route="/contacts"')}`;
      return Torca.components.shell(this.app,{title:t('chats'),subtitle:`${conversations.length} · ${store.state.runtime.provider}`,actions,body});
    }
    bind() {
      Torca.components.bindShell(this.root,this.app);
      this.on('[data-conversation]','click',(event)=>this.app.router.navigate(`/chat/${event.currentTarget.dataset.conversation}`));
      this.on('[data-chat-search]','click',()=>Torca.components.toast('Search mock — M1.7 interaction surface'));
    }
  }
  Torca.screens.ChatsScreen=ChatsScreen;
}(window.Torca));

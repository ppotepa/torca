(function (Torca) {
  'use strict';
  class ChatsScreen extends Torca.core.Screen {
    render() {
      const t=Torca.i18n.t.bind(Torca.i18n); const store=this.app.store;
      const visible=store.state.conversations.filter((c)=>!c.archived&&!c.deleted);
      const list=visible.length?`<div class="conversation-list">${visible.map((c)=>Torca.components.conversationTile(store,c,false)).join('')}</div>`:Torca.components.emptyState('chats',t('noChats'),t('noChatsBody'),Torca.components.button(t('pairContact'),'plus','data-route="/invitations"','primary'));
      const placeholder=Torca.components.emptyState('chats',t('chats'),'Select a conversation to see the timeline.');
      const body=this.app.isCompact()?list:`<div class="split-view"><aside class="list-pane">${list}</aside><section class="content-pane">${placeholder}</section></div>`;
      const actions=`${Torca.components.iconButton('search',t('search'),'data-chat-search')}${Torca.components.iconButton('plus',t('newMessage'),'data-route="/contacts"')}`;
      return Torca.components.shell(this.app,{title:t('chats'),subtitle:`${visible.length} · ${store.state.runtime.provider}`,actions,body});
    }
    bind() {
      Torca.components.bindShell(this.root,this.app);
      this.on('[data-conversation]','click',(event)=>this.app.router.navigate(`/chat/${event.currentTarget.dataset.conversation}`));
      this.on('[data-conversation]','contextmenu',(event)=>{event.preventDefault();Torca.components.showConversationActions(this.app,this,this.app.store.conversation(event.currentTarget.dataset.conversation));});
      this.on('[data-chat-search]','click',()=>Torca.components.showSearch(this.app,{onSelect:(id)=>{const m=this.app.store.state.messages.find((x)=>x.id===id);if(m)this.app.router.navigate(`/chat/${m.conversationId}`);}}));
    }
    conversationAction(action,id){
      if(action==='delete'||action==='archive'){Torca.components.showConfirm({title:'Conversation',message:'Hide this conversation from the chat list?',onConfirm:()=>this.applyConversationAction(action,id)});return;}
      this.applyConversationAction(action,id);
    }
    applyConversationAction(action,id){this.app.store.update((s)=>{const c=s.conversations.find((x)=>x.id===id);if(!c)return;if(action==='mute')c.muted=!c.muted;if(action==='pin')c.pinned=!c.pinned;if(action==='markRead')c.unread=0;if(action==='archive')c.archived=true;if(action==='delete')c.deleted=true;},`conversation-${action}`);Torca.components.toast('Conversation updated');}
  }
  Torca.screens.ChatsScreen=ChatsScreen;
}(window.Torca));

(function (Torca) {
  'use strict';
  class Store {
    constructor(initialState) { this.state = Torca.util.clone(initialState); this.listeners = new Set(); }
    subscribe(listener) { this.listeners.add(listener); return () => this.listeners.delete(listener); }
    emit(reason) { for (const listener of this.listeners) listener(this.state, reason || 'update'); }
    replace(next, reason) { this.state = Torca.util.clone(next); this.emit(reason || 'replace'); }
    update(mutator, reason) { mutator(this.state); this.emit(reason || 'update'); }
    contact(id) { return this.state.contacts.find((item) => item.id === id) || null; }
    conversation(id) { return this.state.conversations.find((item) => item.id === id) || null; }
    messagesFor(conversationId) { return this.state.messages.filter((item) => item.conversationId === conversationId).sort((a,b) => a.createdAt - b.createdAt); }
    unreadTotal() { return this.state.conversations.reduce((sum, item) => sum + (item.unread || 0), 0); }
  }
  Torca.core.Store = Store;
}(window.Torca));

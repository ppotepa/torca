(function (Torca) {
  'use strict';
  class ProfileSetupScreen extends Torca.core.Screen {
    render(){const t=Torca.i18n.t.bind(Torca.i18n);const body=`<div class="screen-scroll"><div class="profile-setup"><div class="identity-glyph">${Torca.components.icon('identity','lg')}</div><h2>${Torca.util.escape(t('createProfile'))}</h2><p>${Torca.util.escape(t('createProfileBody'))}</p><label class="field-label">${Torca.util.escape(t('displayName'))}<input class="text-input" data-profile-name value="Paweł" maxlength="64"></label>${Torca.components.button(t('continue'),'check','data-profile-save','primary')}</div></div>`;return Torca.components.shell(this.app,{title:'Torca',body});}
    bind(){Torca.components.bindShell(this.root,this.app);this.on('[data-profile-save]','click',()=>{const value=this.q('[data-profile-name]').value.trim();if(!value)return;this.app.store.update((s)=>{s.profile.displayName=value;},'profile-created');this.app.router.navigate('/chats');});}
  }
  Torca.screens.ProfileSetupScreen=ProfileSetupScreen;
}(window.Torca));

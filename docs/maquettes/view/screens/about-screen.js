(function (Torca) {
  'use strict';
  class AboutScreen extends Torca.core.Screen {
    render(){const p=this.app.store.state.profile;const r=this.app.store.state.runtime;const t=Torca.i18n.t.bind(Torca.i18n);const body=`<div class="screen-scroll"><div class="page narrow"><div class="about-hero"><div class="brand-mark about-mark">T</div><h2>Torca</h2><p>Private peer-to-peer messenger · ${Torca.util.escape(p.version)}</p></div><div class="card card-pad"><p>${Torca.util.escape(t('aboutAlpha'))}</p></div><div class="card detail-list">${Torca.components.detailRow(t('provider'),r.provider.toUpperCase())}${Torca.components.detailRow(t('providerProfile'),r.providerProfile)}${Torca.components.detailRow(t('build'),r.build)}${Torca.components.detailRow(t('sourceCommit'),p.sourceCommit)}${Torca.components.detailRow('License','AGPL-3.0-or-later')}</div></div></div>`;return Torca.components.shell(this.app,{title:t('about'),leading:Torca.components.iconButton('back','Back','data-route="/chats"'),body});}
    bind(){Torca.components.bindShell(this.root,this.app);this.on('[data-route]','click',(e)=>{e.preventDefault();this.app.router.navigate(e.currentTarget.dataset.route);});}
  }
  Torca.screens.AboutScreen=AboutScreen;
}(window.Torca));

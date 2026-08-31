(function (Torca) {
  'use strict';
  function boot(){const root=document.getElementById('app');const app=new Torca.core.App(root,Torca.fixtures.scenario('normal'));window.torcaMaquette=app;app.start();window.addEventListener('keydown',(event)=>{if(event.key==='~'||event.key==='`'){const bar=document.getElementById('devbar');bar.classList.toggle('is-hidden');document.getElementById('stage').style.paddingTop=bar.classList.contains('is-hidden')?'0':'46px';}});window.addEventListener('resize',()=>{if(app.store.state.ui.viewport==='fluid')app.router.refresh();});}
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',boot);else boot();
}(window.Torca));

# Windows host baseline

The Windows Flutter runner must load `torca_bridge.dll`, own one ClientEngine instance, translate close into minimize-to-tray, restore the existing window on a second launch and flush durable work on real termination.

`host.json` is the committed composition contract. Native runner generation and DLL copy are platform build steps; they are not represented as passing until the owner builds Windows locally.

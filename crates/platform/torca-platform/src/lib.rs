//! Platform lifecycle policy shared by Windows and Android hosts.

/// Host lifecycle event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEvent { Started, Foregrounded, Backgrounded, CloseRequested, Terminating }
/// Engine lifecycle action chosen without platform APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleAction { StartEngine, ResumeEngine, KeepEngineAlive, MinimizeToTray, FlushAndStop, NoOp }
/// Platform class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformClass { WindowsDesktop, AndroidMobile }
/// Deterministic lifecycle policy.
pub struct LifecyclePolicy;
impl LifecyclePolicy {
    /// Maps host lifecycle to engine ownership action.
    pub const fn action(platform: PlatformClass, event: LifecycleEvent) -> LifecycleAction { match (platform, event) { (_, LifecycleEvent::Started) => LifecycleAction::StartEngine, (_, LifecycleEvent::Foregrounded) => LifecycleAction::ResumeEngine, (PlatformClass::WindowsDesktop, LifecycleEvent::CloseRequested) => LifecycleAction::MinimizeToTray, (PlatformClass::AndroidMobile, LifecycleEvent::Backgrounded) => LifecycleAction::KeepEngineAlive, (_, LifecycleEvent::Terminating) => LifecycleAction::FlushAndStop, _ => LifecycleAction::NoOp } }
}

use crate::domain::{DeployPlan, RunTarget, Target};

/// The single normalization boundary shared by interactive and scripted runs.
pub fn normalize(plan: DeployPlan) -> DeployPlan {
    plan.normalized()
}

pub fn all_client_targets() -> Vec<Target> {
    vec![Target::Windows, Target::Android]
}

pub fn all_run_targets() -> Vec<RunTarget> {
    vec![RunTarget::Windows, RunTarget::Android, RunTarget::Emulator]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BuildPolicy, Configuration, DeployAction};

    #[test]
    fn rebuild_requires_a_client_build() {
        let plan =
            DeployPlan::normal(DeployAction::Rebuild, all_client_targets(), Configuration::Debug);
        let plan = normalize(plan);
        assert_eq!(plan.targets, all_client_targets());
        assert_eq!(plan.client_build, BuildPolicy::Rebuild);
    }
}

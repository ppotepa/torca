use crate::domain::{DeployPlan, Target};

/// The single normalization boundary shared by interactive and scripted runs.
pub fn normalize(plan: DeployPlan) -> DeployPlan {
    plan.normalized()
}

pub fn all_client_targets() -> Vec<Target> {
    vec![Target::Windows, Target::Android]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BuildPolicy, Configuration, DeployAction, OnionPolicy};

    #[test]
    fn rotation_expands_to_a_full_client_deployment() {
        let mut plan =
            DeployPlan::normal(DeployAction::RelayMaintenance, Vec::new(), Configuration::Debug);
        plan.onion = OnionPolicy::RotateIdentity;
        let plan = normalize(plan);
        assert_eq!(plan.action, DeployAction::FullRedeploy);
        assert_eq!(plan.targets, all_client_targets());
        assert_eq!(plan.client_build, BuildPolicy::Rebuild);
        assert_eq!(plan.relay_build, BuildPolicy::Rebuild);
    }
}

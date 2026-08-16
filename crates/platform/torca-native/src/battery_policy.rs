use torca_battery::{BatteryPreferences, EffectiveBatteryPolicy, SystemEnergyState};

/// Native-owned battery policy state. Keeping preference/system inputs and
/// effective-policy calculation together prevents lifecycle, diagnostics and
/// startup from drifting apart.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BatteryPolicyState {
    pub preferences: BatteryPreferences,
    pub system: SystemEnergyState,
}

impl BatteryPolicyState {
    pub const fn new(preferences: BatteryPreferences, system: SystemEnergyState) -> Self {
        Self { preferences, system }
    }

    pub fn effective(
        self,
        has_critical_lease: bool,
        diagnostics_override: bool,
    ) -> EffectiveBatteryPolicy {
        self.preferences.effective(self.system, has_critical_lease, diagnostics_override)
    }

    pub fn apply_system_event(&mut self, event: &str) {
        self.system = match event {
            "foregrounded" | "host_started" => self.system.with_foreground(true),
            "backgrounded" => self.system.with_foreground(false),
            "power_saver_on" => self.system.with_power_saver(Some(true)),
            "power_saver_off" => self.system.with_power_saver(Some(false)),
            "charging_on" => self.system.with_charging(Some(true)),
            "charging_off" => self.system.with_charging(Some(false)),
            "metered_network_on" => self.system.with_metered_network(Some(true)),
            "metered_network_off" => self.system.with_metered_network(Some(false)),
            "network_validated" => self.system.with_validated_network(Some(true)),
            "network_unvalidated" => self.system.with_validated_network(Some(false)),
            "data_stall_on" => self.system.with_data_stall(true),
            "data_stall_off" => self.system.with_data_stall(false),
            _ => self.system,
        };
    }
}

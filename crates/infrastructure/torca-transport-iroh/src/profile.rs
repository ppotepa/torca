use torca_transport_api::EnergyClass;

use super::IrohIdentityError;

/// Deployment-time Iroh endpoint policy. This is intentionally provider-local:
/// the generic runtime asks for availability/dormancy, while the endpoint
/// builder decides which discovery and relay services are appropriate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrohEndpointProfile {
    AlwaysReachable,
    DirectOnly,
    LocalOnly,
}

impl IrohEndpointProfile {
    pub fn from_wire(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "direct" | "direct-only" => Self::DirectOnly,
            "local" | "local-only" => Self::LocalOnly,
            _ => Self::AlwaysReachable,
        }
    }

    pub const fn wire_value(self) -> &'static str {
        match self {
            Self::AlwaysReachable => "always",
            Self::DirectOnly => "direct",
            Self::LocalOnly => "local",
        }
    }

    pub const fn energy_class(self) -> EnergyClass {
        match self {
            Self::DirectOnly | Self::LocalOnly => EnergyClass::Low,
            Self::AlwaysReachable => EnergyClass::Medium,
        }
    }

    pub const fn supports_incoming_reachability(self) -> bool {
        matches!(self, Self::AlwaysReachable)
    }

    pub(super) fn from_environment() -> Self {
        option_env!("TORCA_IROH_PROFILE")
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_PROFILE").ok())
            .map(|value| Self::from_wire(&value))
            .unwrap_or(Self::AlwaysReachable)
    }

    pub(super) fn apply(self, builder: iroh::endpoint::Builder) -> iroh::endpoint::Builder {
        match self {
            Self::AlwaysReachable => builder,
            Self::DirectOnly | Self::LocalOnly => {
                builder.clear_address_lookup().relay_mode(iroh::RelayMode::Disabled)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct IrohServiceConfig {
    pub(super) relay_urls: Vec<iroh::RelayUrl>,
    pub(super) pkarr_url: Option<url::Url>,
}

const COMPILED_IROH_RELAY_URLS: Option<&str> = option_env!("TORCA_IROH_RELAY_URLS");
const COMPILED_IROH_PKARR_URL: Option<&str> = option_env!("TORCA_IROH_PKARR_URL");
pub(super) const COMPILED_IROH_DISABLE_RELAY: Option<&str> =
    option_env!("TORCA_IROH_DISABLE_RELAY");
pub(super) const COMPILED_IROH_DISABLE_DISCOVERY: Option<&str> =
    option_env!("TORCA_IROH_DISABLE_DISCOVERY");
pub(super) const COMPILED_IROH_LOCAL_ONLY: Option<&str> = option_env!("TORCA_IROH_LOCAL_ONLY");
pub(super) const COMPILED_IROH_RUNTIME_THREADS: Option<&str> =
    option_env!("TORCA_IROH_RUNTIME_THREADS");

pub(super) fn configured_flag(compiled: Option<&str>, key: &str) -> bool {
    compiled
        .map(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
        })
        .or_else(|| {
            std::env::var(key).ok().map(|value| {
                matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
            })
        })
        .unwrap_or(false)
}

impl IrohServiceConfig {
    pub(super) fn from_environment(
        profile: IrohEndpointProfile,
    ) -> Result<Self, IrohIdentityError> {
        if !profile.supports_incoming_reachability() {
            return Ok(Self::default());
        }
        let relay_value = COMPILED_IROH_RELAY_URLS
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_RELAY_URLS").ok());
        let pkarr_value = COMPILED_IROH_PKARR_URL
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_PKARR_URL").ok());
        Self::from_values(
            profile,
            relay_value.as_deref(),
            pkarr_value.as_deref().filter(|value| !value.trim().is_empty()),
        )
    }

    pub(super) fn from_values(
        profile: IrohEndpointProfile,
        relay_value: Option<&str>,
        pkarr_value: Option<&str>,
    ) -> Result<Self, IrohIdentityError> {
        if !profile.supports_incoming_reachability() {
            return Ok(Self::default());
        }
        let relay_urls = relay_value
            .into_iter()
            .flat_map(|value| {
                value.split(',').map(str::trim).map(str::to_owned).collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<iroh::RelayUrl>().map_err(|error| {
                    IrohIdentityError::Bind(format!(
                        "invalid TORCA_IROH_RELAY_URLS entry '{value}': {error}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pkarr_url = pkarr_value
            .map(str::to_owned)
            .map(|value| {
                value.parse::<url::Url>().map_err(|error| {
                    IrohIdentityError::Bind(format!(
                        "invalid TORCA_IROH_PKARR_URL '{value}': {error}"
                    ))
                })
            })
            .transpose()?;
        Ok(Self { relay_urls, pkarr_url })
    }

    pub(super) fn is_custom(&self) -> bool {
        !self.relay_urls.is_empty() || self.pkarr_url.is_some()
    }
}

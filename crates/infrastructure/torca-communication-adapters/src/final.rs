#[path = "lib.rs"]
mod adapters;
pub use adapters::*;

mod production;
pub use production::{
    CommunicationBuildError, ProductionCommunicationInputs, build_production_communication,
};

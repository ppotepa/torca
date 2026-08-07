#[path = "lib.rs"]
mod adapters;
pub use adapters::*;

mod attachment_controls;
pub use attachment_controls::AttachmentControlAdapter;

mod relationship_admin;
pub use relationship_admin::RelationshipAdminAdapter;

mod production;
pub use production::{
    CommunicationBuildError, ProductionCommunicationInputs, build_production_communication,
};

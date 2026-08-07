#[path = "lib.rs"]
mod adapters;
pub use adapters::*;

mod attachment_controls;
pub use attachment_controls::AttachmentControlAdapter;

mod attachment_export;
pub use attachment_export::AttachmentExportAdapter;

mod relationship_admin;
pub use relationship_admin::RelationshipAdminAdapter;

mod production;
pub use production::{CommunicationBuildError, ProductionCommunicationInputs, build_production_communication};

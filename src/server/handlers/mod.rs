mod commands;
mod completion;
mod definition;
mod diagnostics;
mod hover;
mod inlay_hints;
mod references;
mod rename;
pub(crate) mod util;

pub use commands::handle_execute_command;
pub use completion::handle_completion;
pub use definition::handle_definition;
pub use diagnostics::compute_diagnostics;
pub use hover::handle_hover;
pub use inlay_hints::handle_inlay_hints;
pub use references::{handle_references, handle_workspace_symbol};
pub use rename::{handle_prepare_rename, handle_rename};

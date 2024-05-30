pub mod contract;
mod error;
pub mod helpers;
pub mod msg;
pub mod state;
pub mod testing;

pub use crate::error::ContractError;

pub use contract::interface::CCGovInterface;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CCGOV_NAMESPACE: &str = "ccgov-ns";
pub const CCGOV_NAME: &str = "ccgov";
pub const CCGOV_ID: &str = const_format::formatcp!("{CCGOV_NAMESPACE}:{CCGOV_NAME}");

pub const QUERY_TALLY_CALLBACK_ID: &str = "query_tally_callback";

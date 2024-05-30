use abstract_app::sdk::AbstractSdkError;
use abstract_app::std::AbstractError;
use abstract_app::AppError;
use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Power contract not whitelisted")]
    PowerContractNotWhitelisted {},

    #[error("Voting period for proposal has not ended yet")]
    VotingPeriodNotEnded {},

    #[error("Voting period for proposal has ended")]
    VotingPeriodHasEnded {},

    #[error("Option is not an option of the proposal")]
    InvalidOption {},

    #[error("{0}")]
    Abstract(#[from] AbstractError),

    #[error("{0}")]
    AbstractSdk(#[from] AbstractSdkError),

    #[error("{0}")]
    DappError(#[from] AppError),

    #[error("Voter has already voted")]
    AlreadyVoted {},

    #[error("pre-requisite proposal not resolved")]
    RemoteProposalNotResolved { prereq_prop_ids: Vec<u64> },

    #[error("Unauthorized IBC message")]
    UnauthorizedIbcMessage {},

    #[error("Error while executing IBC message")]
    IBCError {},

    #[error("Proposal not executed yet")]
    ProposalNotExecuted {},
}

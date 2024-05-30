use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::state::{Proposal, Vote};

use crate::contract::CCGovApp;

// This is used for type safety and re-exporting the contract endpoint structs.
abstract_app::app_msg_types!(CCGovApp, CCGovExecuteMsg, CCGovQueryMsg);

#[cw_serde]
pub struct CCGovInstantiateMsg {}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
#[impl_into(ExecuteMsg)]
pub enum CCGovExecuteMsg {
    // Create a new proposal.
    CreateProposal {
        title: String,
        description: String,
        power_contract_addr: String,
        options: Vec<String>,
        // proposal id on remote chain, remote chain id, remote contract address
        prereq_proposals: Vec<(u64, String, String)>,
    },
    // Execute a proposal for which the voting period has ended.
    ExecuteProposal {
        prop_id: u64,
    },
    // Vote on a proposal. Power is calculated according to the power contract.
    Vote {
        prop_id: u64,
        option: String,
    },
}

#[non_exhaustive]
#[cosmwasm_schema::cw_serde]
pub enum CCGovIbcMessage {
    // Route a message
    QueryTally { prop_id: u64 },
}

#[cosmwasm_schema::cw_serde]
pub struct CCGovMigrateMsg {}

#[cosmwasm_schema::cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
#[impl_into(QueryMsg)]
pub enum CCGovQueryMsg {
    #[returns(QueryProposalResponse)]
    QueryProposal { prop_id: u64 },

    #[returns(QueryVoteResponse)]
    QueryVote { prop_id: u64, voter: String },

    #[returns(QueryTotalVotedPowerResponse)]
    QueryTotalVotedPower { prop_id: u64 },

    #[returns(QueryExecutedProposalsResponse)]
    QueryExecutedProposals {},

    #[returns(QueryTallyResponse)]
    QueryTally { prop_id: u64 },
}

#[cosmwasm_schema::cw_serde]
pub struct QueryProposalResponse {
    pub prop: Proposal,
}

#[cosmwasm_schema::cw_serde]
pub struct QueryVoteResponse {
    pub vote: Vote,
}

// The message that needs to be sent to the power contract to get the voting power of a voter.
#[cosmwasm_schema::cw_serde]
pub struct GetVotingPowerMsg {
    pub voter: String,
}

// The response to a GetVotingPowerMsg to a power contract needs to have this form.
#[cosmwasm_schema::cw_serde]
pub struct GetVotingPowerResponse {
    pub power: u64,
}

#[cosmwasm_schema::cw_serde]
pub struct QueryTotalVotedPowerResponse {
    pub power: u64,
}

#[cosmwasm_schema::cw_serde]
pub struct QueryExecutedProposalsResponse {
    pub executed_proposals: Vec<(u64, String)>,
}

#[cosmwasm_schema::cw_serde]
pub struct QueryTallyResponse {
    // option, num_votes
    pub tally: Vec<(String, u64)>,
}

#[cosmwasm_schema::cw_serde]
pub struct RemoteProposalMsg {
    pub prop_id: u64,
    pub remote_chain_id: String,
    pub remote_contract_addr: String,
}

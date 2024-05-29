use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::state::Proposal;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    CreateProposal {
        title: String,
        description: String,
        power_contract_addr: String,
        options: Vec<String>,
    },
    Vote {
        prop_id: u64,
        option: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(QueryProposalResponse)]
    QueryProposal { prop_id: u64 },
}

#[cw_serde]
pub struct QueryProposalResponse {
    prop: Proposal,
}

// The message that needs to be sent to the power contract to get the voting power of a voter.
#[cw_serde]
pub struct GetVotingPowerMsg {
    pub voter: String,
}

#[cw_serde]
pub struct GetVotingPowerResponse {
    pub power: u64,
}

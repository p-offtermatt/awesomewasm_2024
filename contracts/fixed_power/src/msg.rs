use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub struct ExecuteMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetVotingPowerResponse)]
    GetVotingPowerMsg { voter: String },
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

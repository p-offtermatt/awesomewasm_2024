use cosmwasm_schema::cw_serde;
use cosmwasm_std::Timestamp;
use cw_storage_plus::Item;
use cw_storage_plus::Map;

pub const PROP_ID: Item<u64> = Item::new("prop_id");

#[cw_serde]
pub struct Proposal {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub time: Timestamp,
    pub executed: bool,
    pub options: Vec<String>,
    // A contract address that is called to get the power of a voter.
    // Contracts need to be first whitelisted by governance.
    pub power_contract: String,
}

// Proposal ID -> Proposal
pub const PROP_MAP: Map<u64, Proposal> = Map::new("prop_map");

pub const VOTE_ID: Item<u64> = Item::new("vote_id");

#[cw_serde]
pub struct Vote {
    pub id: u64,
    pub prop_id: u64,
    pub voter: String,
    pub power: u64,
    pub option: String,
}

// Voter, Prop_Id -> Vote
pub const VOTE_MAP: Map<(String, u64), Vote> = Map::new("vote_map");

// A whitelist of allowed power contracts that can be set
// as sources of voting power by proposals.
pub const POWER_CONTRACT_WHITELIST: Item<Vec<String>> = Item::new("power_contract_whitelist");

use cosmwasm_schema::cw_serde;
use cosmwasm_std::Timestamp;
use cw_storage_plus::Item;
use cw_storage_plus::Map;

pub const VOTING_PERIOD_IN_MINUTES: Item<u64> = Item::new("voting_period");

pub const PROP_ID: Item<u64> = Item::new("prop_id");

#[cw_serde]
pub struct Proposal {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub start_time: Timestamp,
    pub executed: bool,
    pub options: Vec<String>,
    // A contract address that is called to get the power of a voter.
    // Contracts need to be first whitelisted by governance.
    pub power_contract: String,

    // A list of prerequisite proposals.
    // These are stored as REMOTE_PROPOSAL_IDs on the local chain, see REMOTE_PROPOSALS
    // to see how they are matched to parameters that uniquely identify the proposal on the remote chain.
    pub prereq_proposals: Vec<u64>,
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

// Prop_Id, Voter -> Vote
pub const VOTE_MAP: Map<(u64, String), Vote> = Map::new("vote_map");

// bvector of (proposal id, option that won)
pub const EXECUTED_PROPOSALS: Item<Vec<(u64, String)>> = Item::new("executed_proposals");

// running REMOTE_PROPOSAL_ID
pub const REMOTE_PROPOSAL_ID: Item<u64> = Item::new("remote_proposal_id");

// proposal id of the remote proposal on this chain -> Remote proposal id, remote chain id, remote contract address
pub const REMOTE_PROPOSALS: Map<u64, (u64, String, String)> = Map::new("remote_proposals");

// Remote proposal id on this chain, option -> num_votes
pub const REMOTE_PROPOSALS_TALLIES: Map<(u64, String), u64> = Map::new("remote_proposals_tallies");

// Remote proposal id on this chain -> resolved
pub const REMOTE_PROPOSAL_RESOLVED: Map<u64, bool> = Map::new("remote_proposal_resolved");

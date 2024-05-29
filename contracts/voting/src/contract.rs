#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult,
    WasmQuery,
};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetVotingPowerMsg, GetVotingPowerResponse, InstantiateMsg, QueryMsg};
use crate::state::{
    Proposal, Vote, POWER_CONTRACT_WHITELIST, PROP_ID, PROP_MAP, VOTE_ID, VOTE_MAP,
};

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:ccgov";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    PROP_ID.save(_deps.storage, &0)?;
    VOTE_ID.save(_deps.storage, &0)?;

    Ok(Response::new()
        .add_attribute("action", "initialisation")
        .add_attribute("sender", info.sender.clone()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateProposal {
            title,
            description,
            power_contract_addr,
            options,
        } => {
            // Check if the power contract is whitelisted
            let whitelist = POWER_CONTRACT_WHITELIST.load(deps.storage)?;
            if !whitelist.contains(&power_contract_addr) {
                return Err(ContractError::PowerContractNotWhitelisted {});
            }

            let prop_id = PROP_ID.load(deps.storage)?;
            let prop = Proposal {
                id: prop_id,
                title,
                description,
                time: env.block.time,
                executed: false,
                power_contract: "".to_string(),
                options,
            };
            PROP_MAP.save(deps.storage, prop_id, &prop)?;
            PROP_ID.save(deps.storage, &(prop_id + 1))?;

            Ok(Response::new()
                .add_attribute("action", "create_proposal")
                .add_attribute("prop_id", prop_id.to_string()))
        }
        ExecuteMsg::Vote { prop_id, option } => {
            let vote_id = VOTE_ID.load(deps.storage)?;
            let prop = PROP_MAP.load(deps.storage, prop_id)?;

            // Get the users voting power by querying the power contract
            // specified for the proposal
            let power_msg = to_json_binary(&GetVotingPowerMsg {
                voter: info.sender.clone().to_string(),
            })?;
            let power_response: GetVotingPowerResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: prop.power_contract,
                    msg: to_json_binary(&power_msg)?,
                }))?;

            let vote = Vote {
                id: vote_id,
                prop_id,
                voter: info.sender.clone().to_string(),
                power: power_response.power,
                option,
            };
            VOTE_MAP.save(
                deps.storage,
                (info.sender.clone().to_string(), prop_id),
                &vote,
            )?;

            // increment the vote id
            VOTE_ID.save(deps.storage, &(vote_id + 1))?;

            Ok(Response::new()
                .add_attribute("action", "vote")
                .add_attribute("vote_id", vote_id.to_string()))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg(test)]
mod tests {}

use std::vec;

use abstract_app::objects::module::ModuleInfo;
use abstract_app::sdk::{AbstractResponse, IbcInterface};
use abstract_app::std::ibc::{CallbackResult, ModuleIbcMsg};

use abstract_app::std::ibc_client;
use abstract_client::Namespace;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response,
    StdResult, WasmQuery,
};
use cw_multi_test::Contract;
// use cw2::set_contract_version;

use abstract_app::objects::module::ModuleVersion::Version;

use crate::error::ContractError;
use crate::msg::{
    CCGovExecuteMsg, CCGovIbcMessage, CCGovInstantiateMsg, CCGovMigrateMsg, CCGovQueryMsg,
    GetVotingPowerMsg, GetVotingPowerResponse, QueryExecutedProposalsResponse,
    QueryProposalResponse, QueryTallyResponse, QueryTotalVotedPowerResponse, QueryVoteResponse,
};
use crate::state::{
    Proposal, Vote, EXECUTED_PROPOSALS, PROP_ID, PROP_MAP, REMOTE_PROPOSALS,
    REMOTE_PROPOSALS_TALLIES, REMOTE_PROPOSAL_ID, REMOTE_PROPOSAL_RESOLVED, VOTE_ID, VOTE_MAP,
    VOTING_PERIOD_IN_DAYS,
};
use crate::{APP_VERSION, CCGOV_ID, CCGOV_NAMESPACE};

use abstract_app::AppContract;

pub type CCGovResult<T = Response> = Result<T, ContractError>;

pub type CCGovApp = AppContract<
    ContractError,
    CCGovInstantiateMsg,
    CCGovExecuteMsg,
    CCGovQueryMsg,
    CCGovMigrateMsg,
>;

const APP: CCGovApp = CCGovApp::new(CCGOV_ID, APP_VERSION, None)
    .with_execute(execute_handler)
    .with_query(query_handler)
    .with_dependencies(&[])
    .with_instantiate(instantiate_handler)
    .with_module_ibc(module_ibc_handler);

#[cfg(not(target_arch = "wasm32"))]
impl<Chain: cw_orch::environment::CwEnv> abstract_interface::DependencyCreation
    for crate::CCGovInterface<Chain>
{
    type DependenciesConfig = cosmwasm_std::Empty;
}

// Export handlers
#[cfg(feature = "export")]
abstract_app::export_endpoints!(APP, CCGovApp);

abstract_app::cw_orch_interface!(APP, CCGovApp, CCGovInterface);

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:ccgov";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate_handler(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _app: CCGovApp,
    _msg: CCGovInstantiateMsg,
) -> Result<Response, ContractError> {
    PROP_ID.save(_deps.storage, &0)?;
    VOTE_ID.save(_deps.storage, &0)?;
    REMOTE_PROPOSAL_ID.save(_deps.storage, &0)?;

    VOTING_PERIOD_IN_DAYS.save(_deps.storage, &7)?;

    // set the executed proposals to an empty list
    EXECUTED_PROPOSALS.save(_deps.storage, &Vec::new())?;

    Ok(Response::new()
        .add_attribute("action", "initialisation")
        .add_attribute("sender", info.sender.clone()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute_handler(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    app: CCGovApp,
    msg: CCGovExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        CCGovExecuteMsg::CreateProposal {
            title,
            description,
            power_contract_addr,
            options,
            prereq_proposals,
        } => {
            let prop_id = PROP_ID.load(deps.storage)?;

            PROP_ID.save(deps.storage, &(prop_id + 1))?;

            let mut prereq_ids = vec![];

            // for each proposal in prereq_proposals, create a remote proposal
            for (prereq_prop_id, remote_chain, remote_module_addr) in
                prereq_proposals.iter()
            {
                let remote_proposal_id = REMOTE_PROPOSAL_ID.load(deps.storage)?;
                REMOTE_PROPOSALS.save(
                    deps.storage,
                    prop_id,
                    &(
                        remote_proposal_id,
                        remote_chain.clone(),
                        remote_module_addr.clone(),
                    ),
                )?;
                REMOTE_PROPOSAL_ID.save(deps.storage, &(remote_proposal_id + 1))?;
                prereq_ids.push(remote_proposal_id);
            }

            let prop = Proposal {
                id: prop_id,
                title,
                description,
                start_time: env.block.time,
                executed: false,
                power_contract: power_contract_addr,
                options,
                prereq_proposals: prereq_ids,
            };
            PROP_MAP.save(deps.storage, prop_id, &prop)?;

            Ok(Response::new()
                .add_attribute("action", "create_proposal")
                .add_attribute("prop_id", prop_id.to_string()))
        }
        CCGovExecuteMsg::Vote { prop_id, option } => {
            let vote_id = VOTE_ID.load(deps.storage)?;
            let prop = PROP_MAP.load(deps.storage, prop_id)?;

            // check that the proposal is still open
            let prop_end_time = prop
                .start_time
                .plus_days(VOTING_PERIOD_IN_DAYS.load(deps.storage)?);

            if prop_end_time <= env.block.time {
                return Err(ContractError::VotingPeriodHasEnded {});
            }

            // check that the option is valid
            if !prop.options.contains(&option) {
                return Err(ContractError::InvalidOption {});
            }

            // check that the voter has not already voted
            if VOTE_MAP
                .may_load(deps.storage, (prop_id, info.sender.clone().to_string()))
                .is_ok()
            {
                return Err(ContractError::AlreadyVoted {});
            }

            // Get the users voting power by querying the power contract
            // specified for the proposal
            let power_msg = to_json_binary(&GetVotingPowerMsg {
                voter: info.sender.clone().to_string(),
            })?;
            let power_response: GetVotingPowerResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: prop.power_contract,
                    msg: power_msg,
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
                (prop_id, info.sender.clone().to_string()),
                &vote,
            )?;

            // increment the vote id
            VOTE_ID.save(deps.storage, &(vote_id + 1))?;

            Ok(Response::new()
                .add_attribute("action", "vote")
                .add_attribute("vote_id", vote_id.to_string()))
        }
        CCGovExecuteMsg::ExecuteProposal { prop_id } => {
            let prop = PROP_MAP.load(deps.storage, prop_id)?;
            let prop_end_time = prop
                .start_time
                .plus_days(VOTING_PERIOD_IN_DAYS.load(deps.storage)?);
            if prop_end_time > env.block.time {
                return Err(ContractError::VotingPeriodNotEnded {});
            }

            // ensure that we have received resolutions from all prerequisite proposals
            let mut remote_unresolveds: vec::Vec<u64> = vec![];
            for prereq_prop_id in prop.prereq_proposals.iter() {
                if !REMOTE_PROPOSAL_RESOLVED
                    .load(deps.storage, *prereq_prop_id)
                    .unwrap_or(false)
                {
                    // store that the proposal is unresolved
                    remote_unresolveds.push(*prereq_prop_id);

                    // request the info from the remote chain - we will not be able to resolve this since this goes via IBC,
                    // but we can request the info already and let the user retry once all remote proposals are resolved
                    
                    // create ibc client
                    // load the remote proposal info
                    let (remote_id, remote_chain, remote_contract_addr) = REMOTE_PROPOSALS.load(deps.storage, *prereq_prop_id)?;

                    let wasm_query = WasmQuery::Smart { contract_addr: remote_contract_addr, msg: to_json_binary(&CCGovIbcMessage::QueryTally { prop_id: remote_id })? };

                    app.ibc_client(deps).ibc_query(remote_chain, wasm_query, callback_info)
                    // call query method
                    // create the request with remote contract addr
                    // give it the query message
                }
            }

            if !remote_unresolveds.is_empty() {
                return Err(ContractError::RemoteProposalNotResolved {
                    prereq_prop_ids: remote_unresolveds,
                });
            }

            let mut option_votes = vec![0; prop.options.len()];

            // check that all prerequisite proposals have been executed
            for prereq_prop_id in prop.prereq_proposals.iter() {
                // go over all options
                // load the remote proposal info
                let (remote_prop_id, remote_chain, remote_module_id, remote_module_version) =
                    REMOTE_PROPOSALS.load(deps.storage, prop_id)?;

                for option in prop.options.iter() {
                    // load the tally for the option
                    let tally = REMOTE_PROPOSALS_TALLIES
                        .load(deps.storage, (remote_prop_id, option.clone()))
                        .unwrap_or(0);
                }
            }

            let votes = VOTE_MAP
                .prefix(prop_id)
                .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                .collect::<StdResult<Vec<_>>>()?;
            let mut total_power = 0;
            let mut option_votes = vec![0; prop.options.len()];
            for vote in votes {
                total_power += vote.1.power;
                let option_index = prop
                    .options
                    .iter()
                    .position(|x| *x == vote.1.option)
                    .unwrap();
                option_votes[option_index] += vote.1.power;
            }

            let mut max_votes = 0;
            let mut max_index = 0;
            for (i, votes) in option_votes.iter().enumerate() {
                if *votes > max_votes {
                    max_votes = *votes;
                    max_index = i;
                }
            }

            // load the old list of executed proposals
            let mut executed_proposals = EXECUTED_PROPOSALS.load(deps.storage)?;
            // store it in the executed proposals
            executed_proposals.push((prop_id, prop.options[max_index].clone()));
            EXECUTED_PROPOSALS.save(deps.storage, &executed_proposals)?;

            let result = prop.options[max_index].clone();
            let response = Response::new()
                .add_attribute("action", "execute_proposal")
                .add_attribute("result", result.clone())
                .add_attribute("total_power", total_power.to_string());

            Ok(response)
        }
    }
}

pub fn get_total_voted_power(deps: Deps, prop_id: u64) -> StdResult<u64> {
    let votes = VOTE_MAP
        .prefix(prop_id)
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;
    let mut total_power = 0;
    for vote in votes {
        total_power += vote.1.power;
    }
    Ok(total_power)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query_handler(
    deps: Deps,
    _env: Env,
    app: &CCGovApp,
    msg: CCGovQueryMsg,
) -> CCGovResult<Binary> {
    match msg {
        CCGovQueryMsg::QueryTotalVotedPower { prop_id } => {
            let total_power = get_total_voted_power(deps, prop_id)?;
            Ok(to_json_binary(&QueryTotalVotedPowerResponse {
                power: total_power,
            })?)
        }
        CCGovQueryMsg::QueryProposal { prop_id } => {
            let prop = PROP_MAP.load(deps.storage, prop_id)?;
            Ok(to_json_binary(&QueryProposalResponse { prop: prop })?)
        }
        CCGovQueryMsg::QueryVote { prop_id, voter } => {
            let vote = VOTE_MAP.load(deps.storage, (prop_id, voter))?;
            Ok(to_json_binary(&QueryVoteResponse { vote: vote })?)
        }
        CCGovQueryMsg::QueryExecutedProposals {} => {
            let executed_proposals = EXECUTED_PROPOSALS.load(deps.storage)?;
            Ok(to_json_binary(&QueryExecutedProposalsResponse {
                executed_proposals,
            })?)
        }
        CCGovQueryMsg::QueryTally { prop_id } => query_tally(deps, prop_id),
    }
}

pub fn migrate_handler(
    _deps: DepsMut,
    _env: Env,
    app: CCGovApp,
    _msg: CCGovMigrateMsg,
) -> CCGovResult {
    Ok(app.response("migrate"))
}

pub fn query_tally(deps: Deps, prop_id: u64) -> CCGovResult<Binary> {
    let prop = PROP_MAP.load(deps.storage, prop_id)?;
    let mut option_votes = vec![0; prop.options.len()];

    // check that the proposal was executed, thus the votes are final
    if !prop.executed {
        return Err(ContractError::VotingPeriodNotEnded {});
    }

    let votes = VOTE_MAP
        .prefix(prop_id)
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    for vote in votes {
        let option_index = prop
            .options
            .iter()
            .position(|x| *x == vote.1.option)
            .unwrap();
        option_votes[option_index] += vote.1.power;
    }

    let res = option_votes
        .iter()
        .zip(prop.options.iter())
        .map(|(votes, option)| (option.clone(), *votes))
        .collect::<Vec<_>>();

    Ok(to_json_binary(&QueryTallyResponse { tally: res })?)
}

pub fn module_ibc_handler(
    deps: DepsMut,
    _env: Env,
    app: CCGovApp,
    msg: ModuleIbcMsg,
) -> Result<Response, ContractError> {
    let ccgov_namespace = Namespace::new(CCGOV_NAMESPACE)?;
    cosmwasm_std::ensure_eq!(
        msg.source_module.namespace,
        ccgov_namespace,
        ContractError::Unauthorized {}
    );

    let wrapped_msg = from_json(msg.msg)?;
    match wrapped_msg {
        CCGovIbcMessage::QueryTally { prop_id } => {
            let tally = query_tally(deps.as_ref(), prop_id)?;

            Ok(app.response("module_ibc").set_data(tally))
        }

        _ => Err(ContractError::UnauthorizedIbcMessage {}),
    }
}

pub struct IbcResponseMsg {
    /// The ID chosen by the caller in the `callback_info.id`
    pub id: String,
    /// The msg sent with the callback request.
    /// This is usually used to provide information to the ibc callback function for context
    pub msg: Option<Binary>,
    pub result: CallbackResult,
}

pub fn query_tally_callback(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    app: CCGovApp,
    ibc_msg: IbcResponseMsg,
) -> CCGovResult<Response> {
    match ibc_msg.result {
        CallbackResult::Query { query, result } => 
        {
            result.
        }
        
    }
}

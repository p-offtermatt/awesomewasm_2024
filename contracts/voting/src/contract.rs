use std::vec;

use abstract_app::objects::module::ModuleInfo;
use abstract_app::sdk::{AbstractResponse, IbcInterface};
use abstract_app::std::ibc::{CallbackInfo, CallbackResult, IbcResponseMsg, ModuleIbcMsg};

use abstract_app::std::ibc_client;
use abstract_client::Namespace;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, QueryRequest,
    Response, StdResult, WasmQuery,
};
use cw_multi_test::Contract;
// use cw2::set_contract_version;

use abstract_app::objects::module::ModuleVersion::Version;

use crate::error::ContractError;
use crate::msg::{
    CCGovExecuteMsg, CCGovIbcMessage, CCGovInstantiateMsg, CCGovMigrateMsg, CCGovQueryMsg,
    GetVotingPowerMsg, GetVotingPowerResponse, QueryExecutedProposalsResponse, QueryMsg,
    QueryProposalResponse, QueryTallyResponse, QueryTotalVotedPowerResponse, QueryVoteResponse,
    RemoteProposalMsg,
};
use crate::state::{
    Proposal, Vote, EXECUTED_PROPOSALS, PROP_ID, PROP_MAP, REMOTE_PROPOSALS,
    REMOTE_PROPOSALS_TALLIES, REMOTE_PROPOSAL_ID, REMOTE_PROPOSAL_RESOLVED, VOTE_ID, VOTE_MAP,
    VOTING_PERIOD_IN_MINUTES,
};
use crate::{APP_VERSION, CCGOV_ID, CCGOV_NAMESPACE, QUERY_TALLY_CALLBACK_ID};

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
    .with_module_ibc(module_ibc_handler)
    .with_ibc_callbacks(&[(QUERY_TALLY_CALLBACK_ID, query_tally_callback)]);

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

    VOTING_PERIOD_IN_MINUTES.save(_deps.storage, &1)?;

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
            for (prereq_prop_id, remote_chain, remote_module_addr) in prereq_proposals.iter() {
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
                .plus_minutes(VOTING_PERIOD_IN_MINUTES.load(deps.storage)?);

            if prop_end_time <= env.block.time {
                return Err(ContractError::VotingPeriodHasEnded {});
            }

            // check that the option is valid
            if !prop.options.contains(&option) {
                return Err(ContractError::InvalidOption {});
            }

            // check that the voter has not already voted
            if VOTE_MAP
                .load(deps.storage, (prop_id, info.sender.clone().to_string()))
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
                .plus_minutes(VOTING_PERIOD_IN_MINUTES.load(deps.storage)?);
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
                    let (remote_id, remote_chain, remote_contract_addr) =
                        REMOTE_PROPOSALS.load(deps.storage, *prereq_prop_id)?;

                    let wasm_query = WasmQuery::Smart {
                        contract_addr: remote_contract_addr.clone(),
                        msg: to_json_binary(&QueryMsg::Module(CCGovQueryMsg::QueryTally {
                            prop_id: remote_id,
                        }))?,
                    };

                    let remote_prop_msg = RemoteProposalMsg {
                        prop_id: *prereq_prop_id,
                        remote_chain_id: remote_chain.clone(),
                        remote_contract_addr: remote_contract_addr.clone(),
                    };

                    let callback_info = CallbackInfo::new(
                        QUERY_TALLY_CALLBACK_ID,
                        Some(to_json_binary(&remote_prop_msg)?),
                    );
                    let msg = app.ibc_client(deps.as_ref()).ibc_query(
                        remote_chain,
                        wasm_query,
                        callback_info,
                    )?;

                    return Ok(Response::new()
                        .add_attribute("action", "execute_proposal")
                        .add_attribute("result", "remote_proposal_unresolved")
                        .add_event(Event::new("remote_proposal_unresolved"))
                        .add_message(msg));
                }
            }

            // get the tally for the proposal
            let option_votes = query_tally(deps.as_ref(), prop_id)?;

            // find which option has the most votes
            // make a map from options to their index in the proposal options
            let mut option_index_map = std::collections::HashMap::new();
            for (i, option) in prop.options.iter().enumerate() {
                option_index_map.insert(option.clone(), i);
            }

            let mut max_votes = 0;
            let mut max_index = 0;
            for (option, votes) in option_votes.iter() {
                if *votes > max_votes {
                    max_votes = *votes;
                    max_index = option_index_map.get(option).unwrap().clone();
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
                .add_attribute("result", result.clone());

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
        CCGovQueryMsg::QueryTally { prop_id } => {
            let tally = query_tally(deps, prop_id)?;

            Ok(to_json_binary(&QueryTallyResponse { tally })?)
        }
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

pub fn query_tally(deps: Deps, prop_id: u64) -> Result<Vec<(String, u64)>, ContractError> {
    let prop = PROP_MAP.load(deps.storage, prop_id)?;

    let mut option_votes = vec![0; prop.options.len()];

    // check that all prerequisite proposals have been executed
    for prereq_prop_id in prop.prereq_proposals.iter() {
        for option in prop.options.iter() {
            // load the tally for the option
            let tally = REMOTE_PROPOSALS_TALLIES
                .load(deps.storage, (*prereq_prop_id, option.clone()))
                .unwrap_or(0);

            // add the tally to the total
            let option_index = prop
                .options
                .iter()
                .position(|x| *x == option.clone())
                .unwrap();

            option_votes[option_index] += tally;
        }
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

    let mut max_votes = 0;
    let mut max_index = 0;
    for (i, votes) in option_votes.iter().enumerate() {
        if *votes > max_votes {
            max_votes = *votes;
            max_index = i;
        }
    }

    Ok(prop
        .options
        .iter()
        .zip(option_votes.iter())
        .map(|(option, votes)| (option.clone(), *votes))
        .collect::<Vec<_>>())
}

pub fn module_ibc_handler(
    deps: DepsMut,
    _env: Env,
    app: CCGovApp,
    msg: ModuleIbcMsg,
) -> Result<Response, ContractError> {
    println!("Module IBC Handler: {:?}", msg);
    let wrapped_msg = from_json(msg.msg)?;
    match wrapped_msg {
        QueryMsg::Module(CCGovQueryMsg::QueryTally { prop_id }) => {
            // check that the proposal was executed
            let executed_props = EXECUTED_PROPOSALS.load(deps.storage)?;
            if !executed_props.iter().any(|(id, _)| *id == prop_id) {
                return Err(ContractError::ProposalNotExecuted {});
            }

            let tally = query_tally(deps.as_ref(), prop_id)?;

            let query_tally_response = QueryTallyResponse { tally };

            Ok(app
                .response("module_ibc")
                .set_data(to_json_binary(&query_tally_response)?))
        }

        _ => panic!("Unknown message"),
    }
}

pub fn query_tally_callback(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    app: CCGovApp,
    ibc_msg: IbcResponseMsg,
) -> CCGovResult<Response> {
    let callback_info = ibc_msg.msg.unwrap();
    match from_json::<RemoteProposalMsg>(&callback_info) {
        Ok(remote_prop_msg) => {
            println!("Remote Proposal Msg: {:?}", remote_prop_msg);

            let remote_prop_id = remote_prop_msg.prop_id;

            match ibc_msg.result {
                CallbackResult::Query { query, result } => {
                    // get the first result (there should only ever be one at a time)
                    let unwrapped_res = result.unwrap();
                    let res = unwrapped_res.get(0).unwrap();

                    // get the tally from the response
                    let remote_tally = from_json::<QueryTallyResponse>(&res)?;

                    for (option, votes) in remote_tally.tally.iter() {
                        REMOTE_PROPOSALS_TALLIES.save(
                            deps.storage,
                            (remote_prop_id, option.clone()),
                            &votes,
                        )?;
                    }

                    REMOTE_PROPOSAL_RESOLVED.save(deps.storage, remote_prop_id, &true)?;

                    Ok(app.response("query_tally_callback"))
                }
                CallbackResult::Execute {
                    initiator_msg: _,
                    result: _,
                } => Err(ContractError::UnauthorizedIbcMessage {}),
                CallbackResult::FatalError(_) => Err(ContractError::IBCError {}),
            }
        }
        Err(_) => Err(ContractError::IBCError {}),
    }
}

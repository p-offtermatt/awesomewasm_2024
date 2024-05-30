use std::io::empty;
use std::ops::{Add, Deref, DerefMut};

use abstract_interface::ManagerExecFns;
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{coin, coins, Addr, BlockInfo, Event};
use cw_orch::mock::cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};
use cw_orch::mock::MockBase;
use cw_orch_interchain::{MockBech32InterchainEnv, MockInterchainEnv};
use std::iter::Iterator;

use crate::contract::{execute_handler, instantiate_handler, query_handler};

use crate::msg::{
    CCGovExecuteMsgFns, CCGovInstantiateMsg, CCGovQueryMsgFns, GetVotingPowerResponse,
    InstantiateMsg,
};
use crate::CCGOV_NAMESPACE;
use fixed_power::msg::{GetVotingPowerMsg, InstantiateMsg as FixedPowerInstantiateMsg};

use abstract_client::{AbstractClient, Application, Environment};
use abstract_interchain_tests::setup::ibc_connect_polytone_and_abstract;
use cw_orch_interchain::prelude::*;
use cw_orch_interchain::InterchainEnv;

use cw_orch::prelude::*;

use cw_orch::{anyhow, prelude::*};

use crate::contract::interface::CCGovInterface;
use abstract_app::objects::namespace::Namespace;
use abstract_cw_orch_polytone::Polytone;

struct TestEnv<Env: CwEnv> {
    abs: AbstractClient<Env>,
    app: Application<Env, CCGovInterface<Env>>,
}

pub const A_CHAIN_ID: &str = "harpoon-1";
pub const A_SENDER: &str = "kujira18k2uq7srsr8lwrae6zr0qahpn29rsp7tfassws";

pub const B_CHAIN_ID: &str = "neutron-1";
pub const B_SENDER: &str = "neutron18k2uq7srsr8lwrae6zr0qahpn29rsp7tu2m2ea";

impl<Env: CwEnv> TestEnv<Env> {
    /// Set up the test environment with an Account that has the App installed
    fn setup(env: Env) -> anyhow::Result<TestEnv<Env>> {
        // Create a sender and mock env
        let namespace = Namespace::new(CCGOV_NAMESPACE)?;

        // You can set up Abstract with a builder.
        let abs_client = AbstractClient::builder(env.clone()).build()?;

        // Publish the app
        let publisher = abs_client.publisher_builder(namespace).build()?;
        publisher.publish_app::<CCGovInterface<_>>()?;

        let app = publisher
            .account()
            .install_app::<CCGovInterface<_>>(&CCGovInstantiateMsg {}, &[])?;

        app.account().as_ref().manager.update_settings(Some(true))?; // enable ibc

        Ok(TestEnv {
            abs: abs_client,
            app,
        })
    }

    fn enable_ibc(&self) -> anyhow::Result<()> {
        Polytone::deploy_on(self.abs.environment().clone(), None)?;
        Ok(())
    }
}

#[test]
fn multi_chain_test() -> anyhow::Result<()> {
    let sender = Addr::unchecked("sender_for_all_chains");
    let interchain =
        MockBech32InterchainEnv::new(vec![(A_CHAIN_ID, A_SENDER), (B_CHAIN_ID, B_SENDER)]);

    let a = interchain.chain(A_CHAIN_ID)?;
    let b = interchain.chain(B_CHAIN_ID)?;

    let a_env = TestEnv::setup(a)?;
    let b_env = TestEnv::setup(b)?;

    a_env.enable_ibc()?;
    b_env.enable_ibc()?;

    ibc_connect_polytone_and_abstract(&interchain, B_CHAIN_ID, A_CHAIN_ID)?;
    ibc_connect_polytone_and_abstract(&interchain, A_CHAIN_ID, B_CHAIN_ID)?;

    let a_app = a_env.app;
    let b_app = b_env.app;

    let mut a_fixed_power_addr: Addr = Addr::unchecked("fixed-power");
    {
        let mut app = a_app.get_chain().app.borrow_mut();

        let fixed_power_code = ContractWrapper::new(
            fixed_power::contract::execute,
            fixed_power::contract::instantiate,
            fixed_power::contract::query,
        );
        let fixed_power_code_id = app.store_code(Box::new(fixed_power_code));
        a_fixed_power_addr = app
            .instantiate_contract(
                fixed_power_code_id,
                Addr::unchecked("owner"),
                &FixedPowerInstantiateMsg {},
                &[],
                "FixedPower",
                None,
            )
            .unwrap();
    }

    let mut b_fixed_power_addr: Addr = Addr::unchecked("fixed-power");
    {
        let mut app = b_app.get_chain().app.borrow_mut();

        let fixed_power_code = ContractWrapper::new(
            fixed_power::contract::execute,
            fixed_power::contract::instantiate,
            fixed_power::contract::query,
        );
        let fixed_power_code_id = app.store_code(Box::new(fixed_power_code));
        b_fixed_power_addr = app
            .instantiate_contract(
                fixed_power_code_id,
                Addr::unchecked("owner"),
                &FixedPowerInstantiateMsg {},
                &[],
                "FixedPower",
                None,
            )
            .unwrap();
    }

    // create proposal on chain A
    let create_prop_response = a_app.create_proposal(
        "cosmwasm is awesome".to_string(),
        vec!["approve".to_string(), "reject".to_string()],
        a_fixed_power_addr.to_string(),
        vec![], // no prerequisites
        "test".to_string(),
    );

    // ensure the proposal was created ok
    assert!(create_prop_response.is_ok(), "{:?}", create_prop_response);

    // create proposal on chain b which references chain As proposal
    let create_prop_response = b_app.create_proposal(
        "cosmwasm is awesome".to_string(),
        vec!["approve".to_string(), "reject".to_string()],
        b_fixed_power_addr.to_string(),
        vec![(
            0,
            "harpoon".to_string(),
            a_app.as_instance().address()?.to_string(),
        )],
        "test".to_string(),
    );

    // vote on chain A
    let vote_response = a_app.vote("approve".to_string(), 0);

    // ensure the vote was successful
    assert!(vote_response.is_ok(), "{:?}", vote_response);

    // make time pass on chain A so the proposal can be executed
    a_app.get_chain().wait_seconds(60);

    // vote on chain B
    let vote_response = b_app.vote("reject".to_string(), 0);

    // ensure the vote was successful
    assert!(vote_response.is_ok(), "{:?}", vote_response);

    // make time pass on chain B so the proposal can be executed
    b_app.get_chain().wait_seconds(60);

    // try to execute proposal on chain B
    let execute_proposal_response = b_app.execute_proposal(0)?;

    // should fail because the proposal on chain A was not executed, but fail by setting attribute
    assert!(
        execute_proposal_response.has_event(&Event::new("wasm-remote_proposal_unresolved")),
        "{:?}",
        execute_proposal_response
    );

    interchain.check_ibc(B_CHAIN_ID, execute_proposal_response)?;

    // execute the proposal on chain A
    let execute_proposal_response = a_app.execute_proposal(0)?;

    // should pass because the proposal on chain A was executed
    assert!(
        !(execute_proposal_response.has_event(&Event::new("wasm-remote_proposal_unresolved"))),
        "{:?}",
        execute_proposal_response,
    );

    // now try to execute the proposal on chain B
    let execute_proposal_response = b_app.execute_proposal(0)?;

    interchain.check_ibc(B_CHAIN_ID, execute_proposal_response)?;

    // now try to execute the proposal on chain
    let execute_proposal_response = b_app.execute_proposal(0)?;

    // finally, the proposal was executed
    assert!(
        !(execute_proposal_response.has_event(&Event::new("wasm-remote_proposal_unresolved"))),
        "{:?}",
        execute_proposal_response
    );

    // query the proposal tally in the store
    let mut query_tally_response = b_app.query_tally(0)?;

    // sort the tallies
    query_tally_response.tally.sort();

    let mut expected_tally = vec![("reject".to_string(), 1), ("approve".to_string(), 1)];
    expected_tally.sort();

    assert_eq!(query_tally_response.tally, expected_tally);

    Ok(())
}

#[test]
fn fixed_power_test() -> anyhow::Result<()> {
    let mock = MockBech32::new("mock");

    let env = TestEnv::setup(mock)?;

    env_logger::try_init();
    let app = env.app;

    let sender: Addr = app.get_chain().sender.clone();

    let fixed_power_code = ContractWrapper::new(
        fixed_power::contract::execute,
        fixed_power::contract::instantiate,
        fixed_power::contract::query,
    );
    let mut fixed_power_addr: Addr = Addr::unchecked("fixed-power");
    {
        let mut app2 = app.get_chain().app.borrow_mut();

        let fixed_power_code_id = app2.store_code(Box::new(fixed_power_code));
        fixed_power_addr = app2
            .instantiate_contract(
                fixed_power_code_id,
                Addr::unchecked("owner"),
                &FixedPowerInstantiateMsg {},
                &[],
                "FixedPower",
                None,
            )
            .unwrap();

        let fixed_power_querymsg = GetVotingPowerMsg {
            voter: "voter".to_string(),
        };

        let fixed_power_response: Result<GetVotingPowerResponse, _> = app2
            .wrap()
            .query_wasm_smart(fixed_power_addr.clone(), &fixed_power_querymsg);

        // print the response
        println!("{:?}", fixed_power_response);

        assert!(fixed_power_response.is_ok());
        assert_eq!(
            fixed_power_response.unwrap(),
            GetVotingPowerResponse { power: 1 }
        );
    }
    let create_prop_response = app.create_proposal(
        "cosmwasm is awesome".to_string(),
        vec!["approve".to_string(), "reject".to_string()],
        fixed_power_addr.to_string(),
        vec![], // no prerequisites
        "test".to_string(),
    );

    // ensure the proposal was created ok
    assert!(create_prop_response.is_ok(), "{:?}", create_prop_response);

    // query the proposal
    let query_response = app.query_proposal(0)?;

    // ensure the proposal was created correctly
    assert_eq!(query_response.prop.id, 0);
    assert_eq!(query_response.prop.title, "test");
    assert_eq!(query_response.prop.description, "cosmwasm is awesome");
    assert_eq!(
        query_response.prop.options,
        vec!["approve".to_string(), "reject".to_string()]
    );
    // print the power contract address
    assert_eq!(
        query_response.prop.power_contract,
        fixed_power_addr.to_string()
    );

    // vote on the proposal
    let vote_response = app.vote("approve".to_string(), 0);

    // print the vote response
    println!("{:?}", vote_response);

    // ensure the vote was successful
    assert!(vote_response.is_ok(), "{:?}", vote_response);

    // query the vote
    let query_vote_response = app.query_vote(0, sender.clone().to_string())?;

    // ensure the vote was created correctly
    assert_eq!(query_vote_response.vote.id, 0);
    assert_eq!(query_vote_response.vote.prop_id, 0);
    assert_eq!(query_vote_response.vote.voter, sender.clone());
    assert_eq!(query_vote_response.vote.power, 1);
    assert_eq!(query_vote_response.vote.option, "approve");

    // check the total power of the proposal is 1
    let query_total_power_response = app.query_total_voted_power(0)?;

    // ensure the total power is correct
    assert_eq!(query_total_power_response.power, 1);

    // try to execute the proposal
    let execute_proposal_response = app.execute_proposal(0);

    // ensure the proposal was not executed
    assert!(execute_proposal_response.is_err());

    // advance the block time by 1 minute
    app.get_chain().wait_seconds(60);

    // try to execute the proposal again
    let execute_proposal_response2 = app.execute_proposal(0);

    // ensure the proposal was executed
    assert!(
        execute_proposal_response2.is_ok(),
        "{:?}",
        execute_proposal_response2
    );

    // check that the proposal is in the executed proposals
    let query_executed_proposals_response = app.query_executed_proposals()?;

    // ensure the executed proposals are correct
    assert_eq!(
        query_executed_proposals_response.executed_proposals,
        vec![(0, "approve".to_string())]
    );

    Ok(())
}

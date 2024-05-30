use std::io::empty;
use std::ops::{Add, Deref, DerefMut};

use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{coin, coins, Addr, BlockInfo};
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
pub const B_CHAIN_ID: &str = "neutro-1";

impl TestEnv<MockBase> {
    /// Set up the test environment with an Account that has the App installed
    fn setup(mock: MockBase) -> anyhow::Result<TestEnv<MockBase>> {
        // Create a sender and mock env
        let sender = mock.sender();
        let namespace = Namespace::new(CCGOV_NAMESPACE)?;

        // You can set up Abstract with a builder.
        let abs_client = AbstractClient::builder(mock).build()?;
        // The app supports setting balances for addresses and configuring ANS.
        abs_client.set_balance(sender.clone(), &coins(123, "ucosm"))?;

        // Publish the app
        let publisher = abs_client.publisher_builder(namespace).build()?;
        publisher.publish_app::<CCGovInterface<_>>()?;

        let app = publisher
            .account()
            .install_app::<CCGovInterface<_>>(&CCGovInstantiateMsg {}, &[])?;

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

// #[test]
// fn multi_chain_test -> anyhow::Result<()> {
//     let sender = Addr::unchecked("sender_for_all_chains");
//     let interchain = MockInterchainEnv::new(vec![(A_CHAIN_ID, &sender.clone().to_string()), (B_CHAIN_ID, &sender.clone().to_string())]);

//     let a = interchain.chain(A_CHAIN_ID)?;
//     let b = interchain.chain(B_CHAIN_ID)?;

//     let a_env = TestEnv::setup(a)?;
//     let b_env = TestEnv::setup(b)?;

//     a_env.enable_ibc()?;
//     b_env.enable_ibc()?;

//     ibc_connect_polytone_and_abstract(&interchain, "archway-1", "juno-1")?;
// }

#[test]
fn fixed_power_test() -> anyhow::Result<()> {
    let mock = MockBase::default();

    let a = interchain.chain(A_CHAIN_ID)?;

    let env = TestEnv::setup(a)?;
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
    assert!(vote_response.is_ok());

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

    // advance the block time by 8 days
    app.get_chain().wait_seconds(8 * 24 * 60 * 60);

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

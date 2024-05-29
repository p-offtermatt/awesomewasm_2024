use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::Addr;
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};

use crate::msg::{GetVotingPowerResponse, InstantiateMsg};
use fixed_power::msg::InstantiateMsg as FixedPowerInstantiateMsg;
use fixed_power::msg::QueryMsg as FixedPowerQueryMsg;

#[test]
fn fixed_power_test() {
    let mut app = App::default();

    let code = ContractWrapper::new(execute, instantiate, query);
    let code_id = app.store_code(Box::new(code));

    let addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("owner"),
            &InstantiateMsg {},
            &[],
            "Contract",
            None,
        )
        .unwrap();

    let fixed_power_code = ContractWrapper::new(
        fixed_power::contract::execute,
        fixed_power::contract::instantiate,
        fixed_power::contract::query,
    );
    let fixed_power_code_id = app.store_code(Box::new(fixed_power_code));
    let fixed_power_addr = app
        .instantiate_contract(
            fixed_power_code_id,
            Addr::unchecked("owner"),
            &FixedPowerInstantiateMsg {},
            &[],
            "FixedPower",
            None,
        )
        .unwrap();

    let fixed_power_querymsg = FixedPowerQueryMsg::GetVotingPowerMsg {
        voter: "voter".to_string(),
    };

    let fixed_power_response: Result<GetVotingPowerResponse, _> = app
        .wrap()
        .query_wasm_smart(fixed_power_addr, &fixed_power_querymsg);

    // print the response
    println!("{:?}", fixed_power_response);

    assert!(fixed_power_response.is_ok());
    assert_eq!(
        fixed_power_response.unwrap(),
        GetVotingPowerResponse { power: 1 }
    );
}

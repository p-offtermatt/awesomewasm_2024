use cosmwasm_schema::write_api;

use ccgov::msg::{CCGovExecuteMsg, InstantiateMsg, CCGovQueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: CCGovExecuteMsg,
        query: CCGovQueryMsg,
    }
}

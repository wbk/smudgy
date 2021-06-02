use std::sync::Arc;

use crate::account::Account;

use super::handler::{Handler, Response};

pub struct CreateCharacter {
    account: Arc<Account>,
}

impl CreateCharacter {
    pub fn new(account: Arc<Account>) -> CreateCharacter {
        CreateCharacter { account }
    }
}

impl Handler for CreateCharacter {
    fn handle(&mut self, msg: &str) -> Response {
        "Not implemented".into()
    }
}

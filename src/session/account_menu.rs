use std::sync::Arc;

use crate::account::Account;

use super::{
    create_character::CreateCharacter,
    handler::{Handler, Response},
};

pub struct AccountMenu {
    account: Arc<Account>,
}

impl AccountMenu {
    pub fn new(account: Account) -> AccountMenu {
        AccountMenu {
            account: Arc::new(account),
        }
    }
}

impl Handler for AccountMenu {
    fn preamble(&self) -> Option<String> {
        Some(String::from(
            "\r\nWhich character would you like to play as? \"new\" to create a new one: ",
        ))
    }

    fn handle(&mut self, msg: &str) -> Response {
        match msg {
            "" => self.preamble().unwrap().into(),
            "new" => {
                Response::NewHandler(Box::new(CreateCharacter::new(Arc::clone(&self.account))))
            }
            character_name => {
                // Load this character and enter the game

                format!(
                    "Loading characters is unimplmeneted - you sent {}",
                    character_name
                )
                .into()
            }
        }
    }
}

pub struct Account {
    name: String,
}

impl Account {
    pub fn new() -> Self {
        Account { name: "Test" }
    }

    pub fn from(login: &str, password: &str) -> Account {
        Account {
            name: String::from(login),
        }
    }
}

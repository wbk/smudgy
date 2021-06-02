pub enum Response {
    NewHandler(Box<dyn Handler + Send>),
    Message(String),
}

impl From<String> for Response {
    fn from(str: String) -> Self {
        Self::Message(str)
    }
}

impl From<&str> for Response {
    fn from(str: &str) -> Self {
        Self::Message(String::from(str))
    }
}
pub trait Handler {
    fn handle(&mut self, msg: &str) -> Response;
    fn preamble(&self) -> Option<String> {
        None
    }
}

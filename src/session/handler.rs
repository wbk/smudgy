pub struct Response {
    pub new_handler: Option<Box<dyn Handler + Send>>,
    pub msg: Option<String>,
}
pub trait Handler {
    fn handle(&mut self, msg: &str) -> Response;
}

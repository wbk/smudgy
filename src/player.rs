struct Player {
    session: Weak<Session>,
}

impl Creature for Player {
    fn broadcast(msg: &str) {}
}

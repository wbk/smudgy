pub enum Position {
    Unconscious,
    Sleeping,
    Resting,
    Sitting,
    Standing,
}

pub trait Creature {
    fn broadcast(msg: &str);
}

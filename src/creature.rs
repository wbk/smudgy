pub enum Position {
    Unconscious,
    Sleeping,
    Resting,
    Sitting,
    Standing,
}

pub enum Race {
    Human,
}

pub trait Creature {
    fn broadcast(msg: &str);
}

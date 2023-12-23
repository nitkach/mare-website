use std::fmt::Display;

use serde::Deserialize;

#[repr(i32)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Breed {
    Earth = 0,
    Pegasus = 1,
    Unicorn = 2,
}

impl Display for Breed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let breed = match self {
            Breed::Earth => "Earth",
            Breed::Pegasus => "Pegasus",
            Breed::Unicorn => "Unicorn",
        };

        write!(f, "{breed}")
    }
}

impl From<i32> for Breed {
    fn from(value: i32) -> Self {
        match value {
            0 => Breed::Earth,
            1 => Breed::Pegasus,
            2 => Breed::Unicorn,
            _ => unreachable!(),
        }
    }
}

impl From<Breed> for i32 {
    fn from(value: Breed) -> Self {
        match value {
            Breed::Earth => 0,
            Breed::Pegasus => 1,
            Breed::Unicorn => 2,
        }
    }
}

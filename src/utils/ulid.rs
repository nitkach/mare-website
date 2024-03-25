use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use ulid::Ulid;

#[derive(Debug, Clone, Copy, sqlx::Type)]
pub(crate) struct DbUlid(Ulid);

impl Display for DbUlid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl DbUlid {
    pub(crate) fn get(&self) -> Ulid {
        self.0
    }
}

impl From<String> for DbUlid {
    fn from(value: String) -> Self {
        ulid::Ulid::from_string(&value)
            .map(DbUlid)
            .unwrap_or_else(|err| panic!("Failed to decode ULID from database: {:?}", anyhow!(err)))
    }
}

#[derive(Default, Clone)]
pub(crate) struct DbUlidGen(Arc<Mutex<ulid::Generator>>);

impl DbUlidGen {
    pub(crate) fn generate(&self) -> Ulid {
        let mut generator = self.0.lock().unwrap();

        loop {
            let Ok(ulid) = generator.generate() else {
                std::thread::yield_now();
                continue;
            };

            return ulid;
        }
    }
}

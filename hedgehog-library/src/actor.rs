use crate::datasource::SqliteDataProvider;
use actix::{Actor, Context};

pub struct Library {
    data_provider: SqliteDataProvider,
}

impl Library {
    pub fn new(data_provider: SqliteDataProvider) -> Self {
        Library { data_provider }
    }
}

impl Actor for Library {
    type Context = Context<Self>;
}

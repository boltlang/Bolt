
use crate::files::Files;
use crate::system::System;

#[salsa::db]
pub trait Db : salsa::Database {
    fn system(&self) -> &dyn System;
    fn files(&self) -> &Files;
}


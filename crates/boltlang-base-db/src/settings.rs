//! Settings that apply to every file, like which prelude to load.

use salsa::{Durability, Setter};

/// The package whose prelude files get when nothing else is said.
pub const DEFAULT_PRELUDE: &str = "std";

/// The settings of a database. They're a Salsa input, so that the queries
/// that read them run again when they change, as when the language server
/// gets a new configuration. There's one per database: see [`Settings::get`].
#[salsa::input(singleton, debug)]
pub struct Settings {
    /// The package whose `__prelude__/lib.bolt` every file can use without
    /// importing it, or `None` for no prelude.
    #[returns(ref)]
    pub prelude: Option<String>,
    /// Whether a prelude that can't be found is an error. It's not for the
    /// default prelude, so that a project without `std` still works.
    pub prelude_required: bool,
}

impl Settings {
    /// Create the settings of `db`. They rarely change, so they're stored
    /// with a high durability, which saves checking them after each edit.
    pub fn init<Db: ?Sized + salsa::Database>(db: &Db, prelude: Option<String>, prelude_required: bool) -> Self {
        Settings::builder(prelude, prelude_required).durability(Durability::HIGH).new(db)
    }

    /// Change the prelude, keeping the durability high.
    pub fn change_prelude<Db: salsa::Database>(db: &mut Db, prelude: Option<String>, prelude_required: bool) {
        let settings = Settings::get(db);
        settings.set_prelude(db).with_durability(Durability::HIGH).to(prelude);
        settings.set_prelude_required(db).with_durability(Durability::HIGH).to(prelude_required);
    }
}

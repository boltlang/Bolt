//! What a program can do to the world outside of it, like printing. The
//! evaluator does nothing to the world itself: it asks its [`Host`], so that
//! `bolt run` can print to the terminal, the playground to a pane, and
//! tests can record what a program printed.

use std::{cell::RefCell, rc::Rc};

use crate::ErrorKind;

pub trait Host {
    /// Write `text` and a newline.
    fn print(&mut self, text: &str) -> Result<(), ErrorKind>;
}

/// A host that allows no effects, for code evaluated while compiling. It's
/// the evaluator's default.
pub struct NoEffects;

impl Host for NoEffects {
    fn print(&mut self, _text: &str) -> Result<(), ErrorKind> {
        Err(ErrorKind::Effect("print"))
    }
}

/// A host that prints to the standard output.
pub struct Stdout;

impl Host for Stdout {
    fn print(&mut self, text: &str) -> Result<(), ErrorKind> {
        println!("{text}");
        Ok(())
    }
}

/// A host that keeps what's printed, which its clones can read.
#[derive(Clone, Default)]
pub struct Recorder(Rc<RefCell<String>>);

impl Recorder {
    pub fn output(&self) -> String {
        self.0.borrow().clone()
    }
}

impl Host for Recorder {
    fn print(&mut self, text: &str) -> Result<(), ErrorKind> {
        let mut output = self.0.borrow_mut();
        output.push_str(text);
        output.push('\n');
        Ok(())
    }
}

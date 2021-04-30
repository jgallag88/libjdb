use std::io::Result;

pub trait JavaVirtualMachine {
    // Should this take mut self or should we rely on interior mutability?
    fn all_threads(&self) -> Result<Vec<Box<dyn ThreadReference>>>;
}

// TODO should this be a trait or a struct?
pub trait ThreadReference {
    fn name(&self) -> Result<String>;
}

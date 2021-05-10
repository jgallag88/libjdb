use std::io::Result;

pub trait JavaVirtualMachine {
    // Should this take mut self or should we rely on interior mutability?
    // TODO we limit lifetime of ThreadReference to prevent tcp connection from leaking
    //   important b/c JVM only allows one connection at a time, and we don't want to let people
    //   accidentally leave the connection open
    fn all_threads<'a>(&'a self) -> Result<Vec<Box<dyn ThreadReference + 'a>>>;

    fn can_be_modified(&self) -> bool;

    // TODO what should happen if you try to suspend an hprof? Should it succeed or should you get
    // an error?
    fn suspend(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
}

// TODO should this be a trait or a struct?
pub trait ThreadReference {
    fn name(&self) -> Result<String>;
    fn frames(&self) -> Result<Vec<Box<dyn StackFrame>>>;
}

pub trait StackFrame {
    fn location(&self) -> Result<Box<dyn Location>>;
}

pub trait Location {
    fn line_number(&self) -> Result<Option<u32>>;
    fn method(&self) -> Result<Box<dyn Method>>;
    fn declaring_type(&self) -> Result<Box<dyn ReferenceType>>;
}

pub trait ReferenceType {
    fn name(&self) -> Result<String>;
}

pub trait TypeComponent {
    fn name(&self) -> Result<String>;
}

pub trait Method: TypeComponent {}

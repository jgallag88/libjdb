use std::io::Result;

pub trait JavaVirtualMachine where
    Self::Field: Field,
    Self::Location: Location<Self>,
    Self::Method: Method<Self>,
    Self::ReferenceType: ReferenceType<Self>,
    Self::StackFrame: StackFrame<Self>,
    Self::ThreadReference: ThreadReference<Self>
{
    type Field;
    type Location;
    type Method;
    type ReferenceType;
    type StackFrame;
    type ThreadReference;

    // Should this take mut self or should we rely on interior mutability?
    // TODO we need to limit lifetime of ThreadReference to prevent tcp connection from leaking
    //   important b/c JVM only allows one connection at a time, and we don't want to let people
    //   accidentally leave the connection open. All structs that could have been created by calling
    //   methods on this JavaVirtualMachine (and all the struct that could have been created by those
    //   struct, etc.) need to be dropped when this is dropped.
    //
    // Actually, this isn't good enough
    fn all_threads(&self) -> Result<Vec<Self::ThreadReference>>;

    fn can_be_modified(&self) -> bool;

    // TODO what should happen if you try to suspend an hprof? Should it succeed or should you get
    // an error?
    fn suspend(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
}

// TODO understand why ?Sized is needed here
pub trait ObjectReference<Jvm: JavaVirtualMachine + ?Sized> {
    // TODO delete me? Not sure what the correct thing to return here is
    fn unique_id(&self) -> Result<u64>;
    fn reference_type(&self) -> Result<Box<dyn ReferenceType<Jvm>>>;
}

pub trait ThreadReference<Jvm: JavaVirtualMachine + ?Sized> : ObjectReference<Jvm> {
    fn name(&self) -> Result<String>;
    fn frames(&self) -> Result<Vec<Jvm::StackFrame>>;
}

pub trait StackFrame<Jvm: JavaVirtualMachine + ?Sized> {
    fn location(&self) -> Result<Jvm::Location>;
}

pub trait Location<Jvm: JavaVirtualMachine + ?Sized> {
    fn line_number(&self) -> Result<Option<u32>>;
    fn method(&self) -> Result<Jvm::Method>;
    fn declaring_type(&self) -> Result<Jvm::ReferenceType>;
}

pub trait ReferenceType<Jvm: JavaVirtualMachine + ?Sized> {
    fn name(&self) -> Result<String>;
    fn fields(&self) -> Result<Vec<Jvm::Field>>;
    fn get_value(&self, field: &Jvm::Field) -> Result<Value>;
}

pub trait TypeComponent {
    fn name(&self) -> Result<String>;
}

pub trait Method<Jvm: JavaVirtualMachine + ?Sized>: TypeComponent {}

pub trait Field : TypeComponent {}

pub enum Value {
    Byte(i8),
    Short(i16),
    Integer(i32),
    Long(i64),
    // TODO more stuff goes here
}
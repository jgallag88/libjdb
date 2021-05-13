use libjdb::model::{
    Field, JavaVirtualMachine, Location, ObjectReference, ReferenceType, StackFrame,
    ThreadReference, TypeComponent,
};
use std::io::Result;

fn main() {
    let jvm = libjdb::attach_live("jg-60.dlpxdc.co:12345").unwrap();
    print_stacktraces(&jvm).unwrap();
}

fn print_stacktraces<Jvm: JavaVirtualMachine>(jvm: &Jvm) -> Result<()> {
    if jvm.can_be_modified() {
        jvm.suspend()?;
    }
    for thread in jvm.all_threads()? {
        print_stacktrace::<Jvm>(&thread)?;
    }
    if jvm.can_be_modified() {
        jvm.resume()?;
    }

    Ok(())
}

fn print_stacktrace<Jvm: JavaVirtualMachine>(thread: &Jvm::ThreadReference) -> Result<()> {
    // TODO unique_id is not the same as the thread number, or the nid. How do we get those?
    //let thread_id = thread.all_fields();
    //println!("Reference type for thread: {}", thread.reference_type()?.name()?);
    let mut tid_field = None;
    // TODO use field_by_name() instead of fields(). Also, only need to do this once, not once per thead
    for field in thread.reference_type()?.fields()? {
        if field.name()? == "tid" {
            tid_field = Some(field);
        }
    }
    //let tid = tid_field.map(|f| thread.get_value(&f)?)

    println!("\nThread {}: {}", thread.unique_id()?, thread.name()?);
    for frame in thread.frames()? {
        let location = frame.location()?;
        let line_num = match location.line_number()? {
            Some(n) => format!(":{}", n),
            None => String::new(),
        };
        println!(
            "   {}.{}({})",
            location.declaring_type()?.name()?,
            location.method()?.name()?,
            line_num
        );
    }

    Ok(())
}

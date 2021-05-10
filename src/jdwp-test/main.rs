use libjdb::jdwp::reference_type;
use libjdb::jdwp::thread_reference;
use libjdb::jdwp::virtual_machine;
use libjdb::jdwp::JdwpConnection;
use libjdb::model::ThreadReference;
use std::io::Result;

fn main() {
    let jvm = libjdb::attach_live("localhost:12345").unwrap();
    if jvm.can_be_modified() {
        jvm.suspend().unwrap();
    }
    for thread in jvm.all_threads().unwrap() {
        print_stacktrace2(&*thread);
    }
    if jvm.can_be_modified() {
        jvm.resume().unwrap();
    }
}

fn print_stacktrace2(thread: &dyn ThreadReference) -> Result<()> {
    // TODO print thread id
    println!("\nThread {}: {}", 999999, thread.name()?);
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

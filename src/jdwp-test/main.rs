use libjdb::jdwp::JdwpConnection;
use libjdb::jdwp::virtual_machine;
use libjdb::jdwp::reference_type;
use libjdb::jdwp::thread_reference;

fn main() {
    let j_conn = JdwpConnection::new("localhost:12345").unwrap();
    println!("{:?}", virtual_machine::version(&j_conn).unwrap());
    //let v = virtual_machine::all_classes(&j_conn).unwrap();
    //println!("{:?}", v);
    // TODO we want to be able to accept &str instead of a String, but we want
    // to return a String
    //let v = virtual_machine::classes_by_signature(&j_conn, "LExample;").unwrap();
    //println!("{:?}", v);
    virtual_machine::suspend(&j_conn).unwrap();
    let v = virtual_machine::all_threads(&j_conn).unwrap();
    println!("{:?}", v);
    let v = virtual_machine::all_threads(&j_conn).unwrap();
    for thread_id in v.threads {
        print_stacktrace(thread_id, &j_conn)
    }
    virtual_machine::resume(&j_conn).unwrap();
}

fn print_stacktrace(id: u64, conn: &JdwpConnection) {
        let name_reply = thread_reference::name(conn, id).unwrap();
        println!("Thread {}: {}", id, name_reply.name);
        let frames_reply = thread_reference::frames(conn, id, 0, -1).unwrap();
        for frame in frames_reply.frames {
            let class_sig = reference_type::signature(conn, frame.location.class_id).unwrap().signature;
            let method_name = get_method_name(conn, frame.location.class_id, frame.location.method_id);
            println!("    {}.{}()", signature_to_classname(&class_sig), method_name);
        }
        println!("");
}

fn signature_to_classname(sig: &str) -> String {
    // Assuming this sig is Lfully/qualified/Classname; for now
    let s = sig.trim_start_matches('L').trim_end_matches(';');
    return s.replace('/', ".");
}

fn get_method_name(conn: &JdwpConnection, class_id: u64, method_id: u64) -> String {
    for method in reference_type::methods(conn, class_id).unwrap().methods {
        if method.method_id == method_id {
            return method.name;
        }
    }
    panic!("didn't find method name");
}

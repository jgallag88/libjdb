use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use num_traits::cast::FromPrimitive;
use std::cell::{Cell, RefCell};
use std::convert::TryInto;
use std::io::Result;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::net::ToSocketAddrs;

use crate::model::JavaVirtualMachine;
use crate::model::ThreadReference;

pub struct JdwpConnection {
    stream: RefCell<TcpStream>, // TODO wrap in buffered stream?
    next_id: Cell<u32>,
    field_id_size: u8,
    method_id_size: u8,
    object_id_size: u8,
    reference_type_id_size: u8,
    frame_id_size: u8,
}

impl JdwpConnection {
    pub fn new<A: ToSocketAddrs>(jvm_debug_addr: A) -> Result<Self> {
        let mut stream = TcpStream::connect(jvm_debug_addr)?;
        stream.write_all(b"JDWP-Handshake")?;
        // TODO do we need to flush?
        let mut buf = [0; 128];
        let _n = stream.read(&mut buf)?;
        // TODO check that response is what we expect, correct len, etc.

        let mut conn = JdwpConnection {
            stream: RefCell::new(stream),
            next_id: Cell::new(0),
            // Unfortunately, the JDWP protocol isn't defined entirely
            // statically. After establishing a connection, the client must
            // query the JVM to figure out the size of certain fields that
            // will be sent/recieved in future messages. Set the sizes to zero,
            // but fill them in before we hand the struct to the caller.
            field_id_size: 0,
            method_id_size: 0,
            object_id_size: 0,
            reference_type_id_size: 0,
            frame_id_size: 0,
        };

        let id_sizes = { virtual_machine::id_sizes(&conn)? };
        // TODO check sizes
        conn.field_id_size = id_sizes.field_id_size.try_into().unwrap();
        conn.method_id_size = id_sizes.method_id_size.try_into().unwrap();
        conn.object_id_size = id_sizes.object_id_size.try_into().unwrap();
        conn.reference_type_id_size = id_sizes.reference_type_id_size.try_into().unwrap();
        conn.frame_id_size = id_sizes.frame_id_size.try_into().unwrap();
        println!("field id size: {}", conn.field_id_size);
        println!("frame id size: {}", conn.frame_id_size);
        println!("method id size: {}", conn.method_id_size);
        println!("reference type id size: {}", conn.reference_type_id_size);

        Ok(conn)
    }

    fn execute_cmd(&self, command_set: u8, command: u8, data: &[u8]) -> Result<Vec<u8>> {
        let stream = &mut *self.stream.borrow_mut();
        let id = self.next_id.get();
        self.next_id.set(id + 1);

        let len = data.len() + 11; // 11 is size of header
        stream.write_u32::<BigEndian>(len.try_into().unwrap())?;
        stream.write_u32::<BigEndian>(id)?;
        stream.write_u8(0)?; // Flags
        stream.write_u8(command_set)?;
        stream.write_u8(command)?;
        stream.write_all(data)?;

        let len = stream.read_u32::<BigEndian>()? - 11; // 11 is size of header
        let _id = stream.read_u32::<BigEndian>()?; // TODO check that id is what we expect
        let _flags = stream.read_u8()?; // TODO check response flag
        let error_code = stream.read_u16::<BigEndian>()?;
        if error_code != 0 {
            panic!("Error code: {}", error_code);
        }
        let mut buf = vec![0; len as usize];
        stream.read_exact(&mut buf)?;
        Ok(buf)
    }
}

pub struct JdwpJavaVirtualMachine {
    conn: Rc<JdwpConnection>,
}

impl JdwpJavaVirtualMachine {
    pub fn new(conn: JdwpConnection) -> Self {
        JdwpJavaVirtualMachine {
            conn: Rc::new(conn),
        }
    }
}

impl JavaVirtualMachine for JdwpJavaVirtualMachine {
    fn all_threads(&self) -> Result<Vec<Box<dyn ThreadReference>>> {
        // TODO use iterator/map
        let mut threads = vec![];
        for id in virtual_machine::all_threads(self.conn.as_ref())?.threads {
            let x: Box<dyn ThreadReference> = Box::new(JdwpThreadReference {
                conn: self.conn.clone(),
                thread_id: id,
            });
            threads.push(x);
        }
        Ok(threads)
    }
}

struct JdwpThreadReference {
    conn: Rc<JdwpConnection>,
    thread_id: u64, // TODO should have a threadid type? or is this the thread id type?
}

impl ThreadReference for JdwpThreadReference {
    fn name(&self) -> Result<String> {
        Ok(thread_reference::name(self.conn.as_ref(), self.thread_id)?.name)
    }
}

trait Serialize {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()>;
}

impl Serialize for u8 {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_u8(self)
    }
}

impl Serialize for u16 {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self)
    }
}

impl Serialize for u32 {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_u32::<BigEndian>(self)
    }
}

impl Serialize for i32 {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_i32::<BigEndian>(self)
    }
}

impl Serialize for u64 {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        writer.write_u64::<BigEndian>(self)
    }
}

impl Serialize for &str {
    fn serialize<W: Write>(self, writer: &mut W) -> Result<()> {
        let utf8 = self.as_bytes();
        writer.write_u32::<BigEndian>(utf8.len().try_into().unwrap())?;
        writer.write_all(utf8).unwrap();
        Ok(())
    }
}

trait Deserialize {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self>
    where
        Self: std::marker::Sized;
}

impl Deserialize for u8 {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        reader.read_u8()
    }
}

impl Deserialize for u16 {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        reader.read_u16::<BigEndian>()
    }
}

impl Deserialize for u32 {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        reader.read_u32::<BigEndian>()
    }
}

impl Deserialize for i32 {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        reader.read_i32::<BigEndian>()
    }
}

impl Deserialize for u64 {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        reader.read_u64::<BigEndian>()
    }
}

impl Deserialize for String {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let str_len = reader.read_u32::<BigEndian>()?;

        let mut buf = vec![0; str_len as usize];
        reader.read_exact(&mut buf)?;
        // TODO handle utf8 conversion errors, which will involve changing return
        // type (or maybe using lossy conversion?)
        Ok(String::from_utf8(buf).unwrap())
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let count = reader.read_i32::<BigEndian>()?;
        let mut r = vec![];
        // TODO check > 0 ??
        for _ in 0..count {
            let val: T = Deserialize::deserialize(reader)?;
            r.push(val);
        }
        Ok(r)
    }
}

// TODO move me
use std::rc::Rc;
use std::{error::Error, fmt};

#[derive(Debug)]
struct JdwpError {
    msg: String,
}

impl Error for JdwpError {}

impl fmt::Display for JdwpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

// TODO imports?
fn protocol_err(msg: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        JdwpError {
            msg: format!("JDWP Protocol Error: {}", msg),
        },
    )
}

#[derive(Debug, FromPrimitive)]
pub enum TypeTag {
    Class = 1,
    Interface = 2,
    Array = 3,
}

impl Deserialize for TypeTag {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let val = reader.read_u8()?;
        FromPrimitive::from_u8(val)
            .ok_or_else(|| protocol_err(&format!("{} is not a valid Type Tag", val)))
    }
}

#[derive(Debug)]
pub struct Location {
    pub type_tag: TypeTag,
    pub class_id: u64,  // TODO
    pub method_id: u64, // TODO
    pub location_idx: u64,
}

impl Deserialize for Location {
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Location {
            type_tag: Deserialize::deserialize(reader)?,
            class_id: Deserialize::deserialize(reader)?,
            method_id: Deserialize::deserialize(reader)?,
            location_idx: Deserialize::deserialize(reader)?,
        })
    }
}

// TODO can we de-duplicate the struct/Serialize impl for response and additional types?
// TODO use cmd_set as mod ?
macro_rules! command_set {
    ( set_name: $cmd_set_name:ident;
      set_id: $set_id:expr;
      $(command {
          command_fn: $cmd:ident;
          command_id: $cmd_id:expr;
          args: {
              $( $arg:ident: $arg_ty:ty ),*
          }
          response_type: $resp_name:ident {
              $( $resp_val:ident: $resp_val_ty:ty ),*
          }
          $(
              additional_type: $addn_name:ident {
                  $( $addn_val:ident: $addn_val_ty:ty ),*
              }
          )*
      } )+
    ) => {
        pub mod $cmd_set_name {
            #[allow(unused_imports)]
            use super::{Deserialize, JdwpConnection, Serialize, Location};
            use std::io::{Cursor, Read};
            use std::io::Result;

            $(

            #[derive(Debug)]
            pub struct $resp_name {
                $(
                    pub $resp_val: $resp_val_ty,
                )*
            }

            impl Deserialize for $resp_name {
                #[allow(unused_variables)]
                fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
                    Ok($resp_name {
                        $(
                            $resp_val: Deserialize::deserialize(reader)?,
                        )*
                    })
                }
            }

            $(
                #[derive(Debug)]
                pub struct $addn_name {
                    $(
                        pub $addn_val: $addn_val_ty,
                    )*
                }

                impl Deserialize for $addn_name {
                    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
                        Ok($addn_name {
                            $(
                                $addn_val: Deserialize::deserialize(reader)?,
                            )*
                        })
                    }
                }
            )*

            pub fn $cmd(conn: &JdwpConnection $(, $arg: $arg_ty )* ) -> Result<$resp_name> {
                #[allow(unused_mut)]
                let mut buf = vec![];
                $(
                    $arg.serialize(&mut buf)?;
                )*
                let mut resp_buf = Cursor::new(conn.execute_cmd($set_id, $cmd_id, &buf)?);

                Deserialize::deserialize(&mut resp_buf)
            }
            )+
        }
    };
}

// TODO Link to
// https://docs.oracle.com/en/java/javase/11/docs/specs/jdwp/jdwp-protocol.html
// in docs of generated module

command_set! {
    set_name: virtual_machine;
    set_id: 1;
    command {
        command_fn: version;
        command_id: 1;
        args: {}
        response_type: VersionReply {
            description: String,
            jdwp_major: i32,
            jdwp_minor: i32,
            vm_version: String,
            vm_name: String
        }
    }
    command {
        command_fn: classes_by_signature;
        command_id: 2;
        args: {
            signature: &str
        }
        response_type: ClassesBySignatureReply {
            classes: Vec<ClassesBySignatureReplyClass>
        }
        additional_type: ClassesBySignatureReplyClass {
            ref_type_tag: u8, // TODO could use custom type here
            type_id: u64, // TODO this should be a referenceTypeId
            status: u32 // TODO could use special enum here too
        }
    }
    command {
        command_fn: all_classes;
        command_id: 3;
        args: {}
        response_type: AllClassesReply {
            classes: Vec<AllClassesReplyClass>
        }
        additional_type: AllClassesReplyClass {
            ref_type_tag: u8, // TODO could use custom type here
            type_id: u64, // TODO this should be a referenceTypeId
            signature: String,
            status: u32 // TODO could use special enum here too
        }
    }

    command {
        command_fn: all_threads;
        command_id: 4;
        args: {}
        response_type: AllThreadsReply {
            threads: Vec<u64>  // TODO this should be threadId type
        }
    }
    command {
        command_fn: id_sizes;
        command_id: 7;
        args: {}
        response_type: IdSizesReply {
            field_id_size: i32,
            method_id_size: i32,
            object_id_size: i32,
            reference_type_id_size: i32,
            frame_id_size: i32
        }
    }
    command {
        command_fn: suspend;
        command_id: 8;
        args: {}
        response_type: SuspendReply {} // TODO do we need to define these emtpy replies?
    }
    command {
        command_fn: resume;
        command_id: 9;
        args: {}
        response_type: ResumeReply {}
    }
    command {
        command_fn: exit;
        command_id: 10;
        args: {
            exit_code: i32
        }
        response_type: ExitReply {}
    }
}

command_set! {
    set_name: reference_type;
    set_id: 2;
    command {
        command_fn: signature;
        command_id: 1;
        args: {
            reference_type_id: u64 // TODO this should be reference_type_id type
        }
        response_type: SignatureReply {
            signature: String
        }
    }
    command {
        command_fn: methods;
        command_id: 5;
        args: {
            reference_type_id: u64 // TODO this should be reference_type_id type
        }
        response_type: MethodReply {
            methods: Vec<Method>
        }
        additional_type: Method {
            method_id: u64,  // TODO this should be a methodId type
            name: String,
            signature: String,
            mod_bits: i32
        }
    }
}

command_set! {
    set_name: thread_reference;
    set_id: 11;
    command {
        // TODO is this name good?
        command_fn: name;
        command_id: 1;
        args: {
            thread_id: u64 // TODO this should be threadId type
        }
        response_type: NameReply {
            name: String
        }
    }
    command {
        command_fn: frames;
        command_id: 6;
        args: {
            thread_id: u64, // TODO this should be threadId type
            start_frame: i32,
            length: i32
        }
        response_type: FramesReply {
            frames: Vec<Frame>
        }
        additional_type: Frame {
            frame_id: u64, // TODO this should be a frameId type
            location: Location
            // Remaining fields make up a location, might want to create a distinct Location Type
            //type_tag: u8,
            //class_id: u64,  // TODO this should be a classId type
            //method_id: u64,  // TODO this should be a methodId type
            //location_index: u64
        }
    }
}

//
// HPROF Reference Sources:
//
// [1] There is actual documentation on the HPROF format in the
//     docs of OpenJDK version 6 to 7:
//     http://hg.openjdk.java.net/jdk6/jdk6/jdk/raw-file/tip/src/share/demo/jvmti/hprof/manual.html
//
// [2] For OpenJDK 8 there is a header file provider under
//     src/share/demo/jvmti/hprof/hprof_b_spec.h
//
// [3] Since the above can get ouf of date we look for updates
//     in the format from the actual source code of the latest
//     OpenJDK (version 9 to 14):
//     https://github.com/openjdk/jdk/blob/master/src/hotspot/share/services/heapDumper.cpp
//
// Assumptions:
// - For now we assume that all identifier sizes are 8 bytes (u64).
//   XXX - what does the above assumption means for users? only 64-bit dumps?
//
// XXX - Add other resources JVM and JNI spec.
//
use num_enum::TryFromPrimitive;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::mem;

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum RecordTag {
    Utf8String = 0x01,
    LoadClass = 0x02,
    UnloadClass = 0x03,
    StackFrame = 0x04,
    StackTrace = 0x05,
    AllocSites = 0x06,
    HeapSummary = 0x07,
    StartThread = 0x0A,
    EndThread = 0x0B,
    HeapDump = 0x0C,
    CpuSamples = 0x0D,
    ControlSettings = 0x0E,

    // 1.0.2 Record Tags
    HeapDumpSegment = 0x1C,
    HeapDumpEnd = 0x2C,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum FieldTag {
    ArrayObject = 0x01,
    NormalObject = 0x02,
    Boolean = 0x04,
    Char = 0x05,
    Float = 0x06,
    Double = 0x07,
    Byte = 0x08,
    Short = 0x09,
    Int = 0x0A,
    Long = 0x0B,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum DataDumpSubRecordTag {
    RootUnknown = 0xFF,
    JniGlobal = 0x01,
    JniLocal = 0x02,
    JavaFrame = 0x03,
    NativeStack = 0x04,
    StickyClass = 0x05,
    ThreadBlock = 0x06,
    MonitorUsed = 0x07,
    ThreadObject = 0x08,
    ClassDump = 0x20,
    InstanceDump = 0x21,
    ObjectArrayDump = 0x22,
    PrimitiveArrayDump = 0x23,
}

#[derive(Debug)]
struct Header {
    format: String,
    identifier_size: u32,
    high_word_ms: u32,
    low_word_ms: u32,
}

fn parse_header<R: BufRead>(reader: &mut R) -> Header {
    let mut format_buf = [0u8; 19];
    let mut u32_buf = [0u8; 4];

    reader.read_exact(&mut format_buf).unwrap();
    let format = String::from_utf8_lossy(&format_buf).to_string();
    reader.read_exact(&mut u32_buf).unwrap();
    let identifier_size = u32::from_be_bytes(u32_buf);
    reader.read_exact(&mut u32_buf).unwrap();
    let high_word_ms = u32::from_be_bytes(u32_buf);
    reader.read_exact(&mut u32_buf).unwrap();
    let low_word_ms = u32::from_be_bytes(u32_buf);

    Header {
        format,
        identifier_size,
        high_word_ms,
        low_word_ms,
    }
}

#[derive(Debug)]
struct Record {
    tag: RecordTag,
    time: u32,
    bytes: u32,
}

use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

fn parse_record(parser: &mut HprofParser) -> Record {
    let mut tag_buf = [0u8; 1];
    let mut u32_buf = [0u8; 4];

    parser.reader.read_exact(&mut tag_buf).unwrap();
    let tag = RecordTag::try_from(tag_buf[0]).unwrap();
    parser.reader.read_exact(&mut u32_buf).unwrap();
    let time = u32::from_be_bytes(u32_buf);
    parser.reader.read_exact(&mut u32_buf).unwrap();
    let bytes = u32::from_be_bytes(u32_buf);

    match tag {
        RecordTag::Utf8String => {
            let r: Utf8StringRecord = parser.parse_utf8_string_record(bytes as usize);
            parser.strings_tab.insert(r.identifier, r.value); // XXX
        }
        RecordTag::LoadClass => {
            let r: LoadClassRecord = parser.parse_load_class_record();
            parser.class_tab.insert(r.serial_num, r);
        }
        RecordTag::UnloadClass => {
            // TODO:
            // These currently seem to be non-existent. Once you finish
            // reading the rest of the dump data, if you still don't see
            // such entries then check the C++ Dumper code to see if they
            // are mentioned at all. You probably still want to leave the
            // parsing code here for completeness but should be ok to
            // leave things simplified.
            let _r: UnloadClassRecord = parser.parse_unload_class_record();
        }
        RecordTag::StackFrame => {
            let r: StackFrameRecord = parser.parse_stack_frame_record();
            parser.frame_tab.insert(r.frame_id, r); // XXX
        }
        RecordTag::StackTrace => {
            let _r: StackTraceRecord = parser.parse_stack_trace_record();
            //
            // XXX - The following code is just for exploration and debugging.
            //       It will be removed soon.
            //
            // let r: StackTraceRecord = parse_stack_trace_record(reader);
            // println!("Thread {}:", r.thread_serial_num);
            // for frame_id in r.frame_ids {
            //     let frame = frame_table.get(&frame_id).unwrap();
            //     let class = class_table.get(&frame.class_serial_num).unwrap();
            //     //
            //     // For whatever reason class names read from the HPROF use slashes (/)
            //     // instead of dots (.) for their classpath [e.g. java/lang/Thread.run()
            //     // instead of java.lang.Thread.run()].
            //     //
            //     let class_name = string_table
            //         .get(&class.strname_id)
            //         .unwrap()
            //         .replace("/", ".");
            //     let method_name = string_table.get(&frame.method_name_id).unwrap();
            //     if frame.source_name_id != 0 {
            //         println!(
            //             "\t{}.{}() [{}:{}]",
            //             class_name,
            //             method_name,
            //             string_table.get(&frame.source_name_id).unwrap(),
            //             frame.line_num
            //         );
            //     } else if frame.line_num == -1 {
            //         println!("\t{}.{}() [Unknown]", class_name, method_name);
            //     } else if frame.line_num == -2 {
            //         // XXX: Haven't seen that yet, potentially unimplemented
            //         println!("\t{}.{}() [Compiled]", class_name, method_name);
            //         println!("{:?}", frame);
            //     } else if frame.line_num == -3 {
            //         // XXX: Haven't seen that yet, potentially unimplemented
            //         println!("\t{}.{}() [Native]", class_name, method_name);
            //         println!("{:?}", frame);
            //     } else {
            //         // XXX: skip here maybe with a debug msg
            //         println!("{:?}", frame);
            //     }
            // }
            // println!();
        }
        RecordTag::HeapDump => {
            // parse_heap_dump_records(parser, bytes);
            println!("HeapDump Record Under Construction!");
        }
        _ => {
            println!("tag: {:?} of size {:?} bytes", tag, bytes);
        }
    }
        // XXX: For Testing
    Record { tag, time, bytes }
}

#[derive(Debug)]
struct Utf8StringRecord {
    // XXX: Assumption
    identifier: u64,
    value: String,
}

#[derive(Debug)]
struct LoadClassRecord {
    serial_num: u32,
    // XXX: Assumption?
    object_id: u64,
    strace_num: u32,
    // XXX: Assumption?
    strname_id: u64,
}

#[derive(Debug)]
struct UnloadClassRecord {
    serial_num: u32,
}

#[derive(Debug)]
struct StackFrameRecord {
    frame_id: u64,       // XXX: Assumption
    method_name_id: u64, // XXX: Assumption
    method_sign_id: u64, // XXX: Assumption
    source_name_id: u64, // XXX: Assumption
    class_serial_num: u32,
    line_num: i32,
}

#[derive(Debug)]
struct StackTraceRecord {
    serial_num: u32,
    thread_serial_num: u32,
    nframes: u32,
    frame_ids: Vec<u64>, // XXX: Assumption
}

#[allow(dead_code)]
fn parse_heap_dump_records(parser: &mut HprofParser, dump_segment_size: u32) {
    let dump_segment_start = parser.reader.seek(SeekFrom::Current(0)).unwrap();
    let dump_segment_end = dump_segment_start + u64::from(dump_segment_size);
    let mut current_position = dump_segment_start;

    let mut cd_n: u64 = 0;
    let mut id_n: u64 = 0;
    let mut oad_n: u64 = 0;
    let mut pad_n: u64 = 0;
    while current_position < dump_segment_end {
        let subtag = parser.parse_subrecord_tag();
        match subtag {
            DataDumpSubRecordTag::ClassDump => {
                parse_class_subrecord(parser);
                cd_n += 1;
            }
            DataDumpSubRecordTag::InstanceDump => {
                parse_instance_subrecord(parser);
                id_n += 1;
            }
            DataDumpSubRecordTag::ObjectArrayDump => {
                parse_object_array_subrecord(parser);
                oad_n += 1;
            }
            DataDumpSubRecordTag::PrimitiveArrayDump => {
                parse_primitive_array_subrecord(parser);
                pad_n += 1;
            }
            _ => {
                break;
            }
        }
        current_position = parser.reader.seek(SeekFrom::Current(0)).unwrap();
    }
    println!(
        "current_pos ({}) vs segment_end ({})",
        current_position, dump_segment_end
    );
    println!("sub-entries: {} class {} instance {} obj array {} p array", cd_n, id_n, oad_n, pad_n);
}

// The above is super slow as is...
//
// sub tag: ThreadObject
// current_pos (3116808688) vs segment_end (3117518676)
// sub-entries: 32542 class 45628477 instance 2202270 obj array 3739074 p array
// entries: 394711 string 34189 load 0 unload 25359 frame 1317 trace 1 heapdump
//
// real 19m56.328s
// user 1m8.314s
// sys  4m41.148s
//

#[allow(dead_code)]
fn parse_primitive_array_subrecord(parser: &mut HprofParser) {
    let _array_object_id = parser.parse_u64(); // XXX: Assume
    let _strace_serial_num = parser.parse_u32();
    let n_elements = parser.parse_u32();
    let element_type = parser.parse_field_type_tag();

    // TODO - parse properly
    let element_bytes = match element_type {
        // XXX - Mention Reference Here For Sizes
        FieldTag::Boolean => 1,
        FieldTag::Byte => 1,
        FieldTag::Char => 2,
        FieldTag::Double => 8,
        FieldTag::Float => 4,
        FieldTag::Int => 4,
        FieldTag::Long => 8,
        FieldTag::NormalObject => 8,
        FieldTag::Short => 2,
        _ => {panic!()}
    };
    let _off = parser.reader.seek(SeekFrom::Current(i64::from(n_elements * element_bytes))).unwrap();
}

#[allow(dead_code)]
fn parse_object_array_subrecord(parser: &mut HprofParser) {
    let _array_object_id = parser.parse_u64(); // XXX: Assume
    let _strace_serial_num = parser.parse_u32();
    let n_elements = parser.parse_u32();
    let _array_class_object_id = parser.parse_u64(); // XXX: Assume

    // TODO: elements
    // XXX: Assume
    let _off = parser.reader.seek(SeekFrom::Current(i64::from(n_elements * 8))).unwrap();
}

#[allow(dead_code)]
fn parse_instance_subrecord(parser: &mut HprofParser) {
    let _object_id = parser.parse_u64(); // XXX: Assume
    let _strace_serial_num = parser.parse_u32();
    let _class_object_id = parser.parse_u64(); // XXX: Assume
    let bytes_left = parser.parse_u32();

    // TODO: Parse instance fields
    let _off = parser.reader.seek(SeekFrom::Current(i64::from(bytes_left))).unwrap();
}

#[allow(dead_code)]
fn parse_class_subrecord(parser: &mut HprofParser) {
    let _class_object_id = parser.parse_u64();
    let _strace_serial_num = parser.parse_u32();
    let _superclass_object_id = parser.parse_u64();
    let _class_loader_object_id = parser.parse_u64();
    let _signers_object_id = parser.parse_u64();
    let _pdomain_object_id = parser.parse_u64();

    let _reserved0 = parser.parse_u64();
    let _reserved1 = parser.parse_u64();

    let _instance_size_bytes = parser.parse_u32();

    let constant_pool_size = parser.parse_u16();
    for _ in 0..constant_pool_size {
        // XXX - implement - BYTES!
        println!("CONSTANT_POOL_SIZE IS POPULATED! -> {}", constant_pool_size);
        return;
    }

    let static_field_num = parser.parse_u16();
    for _ in 0..static_field_num {
        let _field_name_id = parser.parse_u64();
        let field_type = parser.parse_field_type_tag();
        match field_type {
            // XXX - Mention Reference Here For Sizes
            FieldTag::Boolean => {
                let _val = parser.parse_u8();
            }
            FieldTag::Byte => {
                let _val = parser.parse_i8();
            }
            FieldTag::Char => {
                let _val = parser.parse_u16();
            }
            FieldTag::Double => {
                // XXX: May need parse_double();
                let _val = parser.parse_u64();
            }
            FieldTag::Float => {
                // XXX: May need parse_float();
                let _val = parser.parse_u32();
            }
            FieldTag::Int => {
                let _val = parser.parse_i32();
            }
            FieldTag::Long => {
                let _val = parser.parse_i64();
            }
            FieldTag::NormalObject => {
                // XXX: Assumption?
                let _val = parser.parse_u64();
            }
            FieldTag::Short => {
                let _val = parser.parse_i16();
            }
            _ => {
                println!("{:?}", field_type);
                return;
            }
        }
    }

    let instance_field_num = parser.parse_u16();
    for _ in 0..instance_field_num {
        let _field_name_id = parser.parse_u64();
        let _field_type = parser.parse_field_type_tag();
    }
}

#[derive(Debug)]
struct HprofParser {
    reader: BufReader<File>,
    header: Header,
    strings_tab: HashMap<u64, String>,
    frame_tab: HashMap<u64, StackFrameRecord>,
    class_tab: HashMap<u32, LoadClassRecord>,
}

impl HprofParser {
    fn new(path: &str) -> HprofParser {
        let f = File::open(path).expect("XXX: file not found?");
        let mut r = BufReader::new(f);
        let h = parse_header(&mut r);
        HprofParser {
            reader: r,
            header: h,
            strings_tab: HashMap::new(),
            frame_tab: HashMap::new(),
            class_tab: HashMap::new(),
        }
    }

    fn done_parsing(&mut self) -> bool {
        if self.reader.fill_buf().unwrap().len() == 0 {
            return true;
        }
        return false;
    }

    #[allow(dead_code)]
    fn parse_subrecord_tag(&mut self) -> DataDumpSubRecordTag {
        DataDumpSubRecordTag::try_from(self.parse_u8()).unwrap()
    }

    #[allow(dead_code)]
    fn parse_field_type_tag(&mut self) -> FieldTag {
        FieldTag::try_from(self.parse_u8()).unwrap()
    }

    #[allow(dead_code)]
    fn parse_i8(&mut self) -> i8 {
        let mut u8_buf = [0u8; 1];
        self.reader.read_exact(&mut u8_buf).unwrap();
        // TODO - XXX - double check below
        i8::from_be(u8_buf[0] as i8)
    }

    #[allow(dead_code)]
    fn parse_u8(&mut self) -> u8 {
        let mut u8_buf = [0u8; 1];
        self.reader.read_exact(&mut u8_buf).unwrap();
        u8_buf[0]
    }

    #[allow(dead_code)]
    fn parse_i16(&mut self) -> i16 {
        let mut u16_buf = [0u8; 2];
        self.reader.read_exact(&mut u16_buf).unwrap();
        i16::from_be_bytes(u16_buf)
    }

    #[allow(dead_code)]
    fn parse_u16(&mut self) -> u16 {
        let mut u16_buf = [0u8; 2];
        self.reader.read_exact(&mut u16_buf).unwrap();
        u16::from_be_bytes(u16_buf)
    }

    fn parse_i32(&mut self) -> i32 {
        let mut u32_buf = [0u8; 4];
        self.reader.read_exact(&mut u32_buf).unwrap();
        i32::from_be_bytes(u32_buf)
    }

    fn parse_u32(&mut self) -> u32 {
        let mut u32_buf = [0u8; 4];
        self.reader.read_exact(&mut u32_buf).unwrap();
        u32::from_be_bytes(u32_buf)
    }

    #[allow(dead_code)]
    fn parse_i64(&mut self) -> i64 {
        let mut u64_buf = [0u8; 8];
        self.reader.read_exact(&mut u64_buf).unwrap();
        i64::from_be_bytes(u64_buf)
    }

    fn parse_u64(&mut self) -> u64 {
        let mut u64_buf = [0u8; 8];
        self.reader.read_exact(&mut u64_buf).unwrap();
        u64::from_be_bytes(u64_buf)
    }

    fn parse_utf8_string(&mut self, bytes: usize) -> String {
        let mut value_buf = vec![0u8; bytes];
        self.reader.read_exact(&mut value_buf).unwrap();
        String::from_utf8_lossy(&value_buf).to_string()
    }

    fn parse_utf8_string_record(&mut self, bytes: usize) -> Utf8StringRecord {
        let identifier = self.parse_u64();
        let value = self.parse_utf8_string(bytes - mem::size_of::<u64>());
        Utf8StringRecord { identifier, value }
    }

    fn parse_load_class_record(&mut self) -> LoadClassRecord {
        let serial_num = self.parse_u32();
        let object_id = self.parse_u64();
        let strace_num = self.parse_u32();
        let strname_id = self.parse_u64();
        LoadClassRecord {
            serial_num,
            object_id,
            strace_num,
            strname_id,
        }
    }
        fn parse_unload_class_record(&mut self) -> UnloadClassRecord {
        UnloadClassRecord {
            serial_num: self.parse_u32(),
        }
    }

    fn parse_stack_frame_record(&mut self) -> StackFrameRecord {
        let frame_id = self.parse_u64();
        let method_name_id = self.parse_u64();
        let method_sign_id = self.parse_u64();
        let source_name_id = self.parse_u64();
        let class_serial_num = self.parse_u32();
        let line_num = self.parse_i32();

        StackFrameRecord {
            frame_id,
            method_name_id,
            method_sign_id,
            source_name_id,
            class_serial_num,
            line_num,
        }
    }

    fn parse_stack_trace_record(&mut self) -> StackTraceRecord {
        let serial_num = self.parse_u32();
        let thread_serial_num = self.parse_u32();
        let nframes = self.parse_u32();

        let mut frame_ids = vec![0u64; nframes as usize];
        for n in 0..nframes {
            frame_ids[n as usize] = self.parse_u64();
        }

        StackTraceRecord {
            serial_num,
            thread_serial_num,
            nframes,
            frame_ids,
        }
    }
}

fn parse_hprof_file(filename: &str) {
    let mut parser = HprofParser::new(filename);

    // XXX: Debug
    let mut i: u64 = 0;
    let mut j: u64 = 0;
    let mut k: u64 = 0;
    let mut l: u64 = 0;
    let mut m: u64 = 0;
    let mut n: u64 = 0;

    loop {
        if parser.done_parsing() {
            break;
        }
        let record: Record = parse_record(&mut parser);
        match record.tag {
            RecordTag::Utf8String => {
                i += 1;
            }
            RecordTag::LoadClass => {
                j += 1;
            }
            RecordTag::UnloadClass => {
                k += 1;
            }
            RecordTag::StackFrame => {
                l += 1;
            }
            RecordTag::StackTrace => {
                m += 1;
            }
            RecordTag::HeapDump => {
                n += 1;
                break;
            }
            _ => {
                break;
            }
        }
    }

    // XXX: Debug
    println!(
        "entries: {} string {} load {} unload {} frame {} trace {} heapdump",
        i, j, k, l, m, n
    );
}

pub fn sample_fn() {
    let args: Vec<String> = std::env::args().collect();
    match args.len() {
        1 => {
            println!("usage: {} <hprof dump>", args[0]);
        }
        2 => {
            println!("Analyzing {} ...", args[1]);
            parse_hprof_file(&args[1]);
        }
        _ => {
            println!("usage: {} <hprof dump>", args[0]);
        }
    }
}

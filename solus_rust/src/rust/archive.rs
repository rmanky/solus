mod data;
mod gemini;

#[macro_use]
extern crate log;
extern crate android_logger;
use jni::sys::{ jboolean, JNI_TRUE };
use jni::JNIEnv;
use jni::objects::{ JClass, JString };
use log::LevelFilter;
use android_logger::Config;
use proto::proto::message::chat_response::Type;
use serde::Deserialize;
use tokio::runtime::Runtime;
use std::os::unix::net::{ UnixListener, SocketAddr, UnixStream };
use std::os::android::net::SocketAddrExt;
use std::io::{ BufReader, BufWriter, Read, Write };
use std::sync::Arc;
use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };
use proto::proto::message::{ ChatRequest, ChatResponse };
use prost::Message;
use crate::data::CommandData;

struct ChatError {
    message: String,
}

#[derive(Deserialize)]
struct ReplicateSubmit {
    id: String,
}

#[derive(Deserialize)]
struct ReplicatePoll {
    status: String,
    output: Option<Vec<String>>,
}

#[no_mangle]
pub extern "system" fn Java_com_rmanky_solus_Native_startRustServer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_replicate_key: JString<'local>,
    j_gemini_key: JString<'local>
) -> jboolean {
    log_panics::init();
    android_logger::init_once(
        Config::default().with_max_level(LevelFilter::Trace) // Adjust log level as needed
    );

    let addr = SocketAddr::from_abstract_name(b"com.rmanky.solus.socket").unwrap();

    debug!(target: "RustTag", "startRustServer()");
    let listener = UnixListener::bind_addr(&addr).unwrap();
    debug!(target: "RustTag", "bound listener");
    let replicate_key: String = env
        .get_string(&j_replicate_key)
        .expect("Failed to get replicate key!")
        .into();
    let gemini_key: String = env
        .get_string(&j_gemini_key)
        .expect("Failed to get gemini key!")
        .into();

    std::thread::spawn(move || {
        let command_data = Arc::new(CommandData {
            reqwest_client: reqwest::Client::new(),
            replicate_token: replicate_key,
            gemini_token: gemini_key,
        });

        let rt = Arc::new(Runtime::new().unwrap());

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    rt.spawn(handle_client(command_data.clone(), stream, rt.clone()));
                }
                Err(err) => {
                    debug!(target: "RustTag", "Error: {}", err);
                    break;
                }
            }
        }
    });

    debug!(target: "RustTag", "eof!!!");
    JNI_TRUE
}

async fn handle_client(command_data: Arc<CommandData>, stream: UnixStream, rt: Arc<Runtime>) {
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    loop {
        let message_length = reader.read_i32::<BigEndian>().unwrap(); // Read the length prefix
        debug!("Receiving message of length {}", message_length);

        let mut buf = vec![0; message_length as usize];
        let _ = reader.read_exact(&mut buf);
        let chat_request = ChatRequest::decode(&buf[..]).unwrap();

        let prompt = chat_request.prompt;
        let id = chat_request.id;

        let response = match gemini::invoke(command_data.as_ref(), &prompt).await {
            Ok(v) => v,
            Err(e) => todo!(),
        };

        let mut chat_response = ChatResponse::default();
        chat_response.message = response;
        chat_response.id = id;
        chat_response.set_type(Type::End);

        let serialized_response = chat_response.encode_to_vec();
        let response_length = serialized_response.len() as i32;
        debug!("Sending message of length {}", response_length);

        // Write the length prefix and the serialized response
        let _ = writer.write_i32::<BigEndian>(response_length);
        let _ = writer.write(&serialized_response);
        let _ = writer.flush();
    }
}

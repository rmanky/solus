use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&["src/rust/proto/message.proto"], &["src/rust/"])?;
    Ok(())
}

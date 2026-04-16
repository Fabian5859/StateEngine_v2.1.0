fn main() {
    // Esto le dice a Cargo que compile el archivo .proto antes de compilar el bot
    prost_build::compile_protos(&["src/feeds/cqg/proto/cqg_market_data.proto"], &["src/feeds/cqg/proto/"]).unwrap();
}

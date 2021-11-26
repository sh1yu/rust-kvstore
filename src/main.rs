use rust_kvstore::pb::*;
use prost::Message;

fn main() {
    let request = RequestGet {
        key: "hello".to_string(),
    };
    let mut buf = Vec::new();
    request.encode(&mut buf).unwrap();
    println!("encoded: {:?}", buf);
}

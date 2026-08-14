mod payload_engine;

fn main() {
    println!("Hello, world!");
    let filename = "example.txt";
    let password = "1234";
    let processed_payload =
        payload_engine::prep_payload(filename, password).expect("failed to prepare");
    println!("Payload processed successfully");

    let recovery_result = payload_engine::parse_payload(&processed_payload, 256)
        .expect("Failed to recover the result");
    println!("Recovered Filename: {}", recovery_result.filename);
    println!("Recovered File Bytes: {}", recovery_result.file_bytes.len());

    if let Ok(text) = String::from_utf8(recovery_result.file_bytes) {
        println!("Recovered File Content Preview:\n{}", text);
    }
}

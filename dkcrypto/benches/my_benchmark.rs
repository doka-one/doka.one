use criterion::{Criterion, criterion_group, criterion_main};

use dkcrypto::dk_crypto::DkEncrypt;

pub fn d10_performance() {
    let phrases = [
        "Hello, how are you?",
        "I love coding in Rust.",
        "The quick brown fox jumps over the lazy dog.",
        "Rust is a systems programming language.",
        "OpenAI's GPT-3.5 is an amazing language model.",
        "I enjoy helping people with their questions.",
        "Rustaceans are a friendly community.",
        "Programming is fun and challenging.",
        "The beach is a great place to relax.",
        "I like to read books in my free time.",
        "Learning new things is always exciting.",
        "Rust is known for its memory safety features.",
        "I'm excited to see what the future holds.",
        "The mountains are majestic and beautiful.",
        "I enjoy playing musical instruments.",
        "Coding allows us to create amazing things.",
        "Rust's syntax is elegant and expressive.",
        "I believe in lifelong learning.",
        "The stars are mesmerizing at night.",
        "Rust enables high-performance software development.",
        "I'm grateful for the opportunities I have."
    ];

    for phrase in phrases.iter() {
        let _encrypted = DkEncrypt::encrypt_str(phrase, KEY).unwrap();
    }
}

const KEY: &str = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs";

fn encrypt_test(phrase : &str, key: &str) {
        let _ = DkEncrypt::encrypt_str(phrase, &key).unwrap();
}

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n-1) + fibonacci(n-2),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("encrypt 1", |b| b.iter(|| d10_performance() ));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
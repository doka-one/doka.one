use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dkcrypto::dk_crypto::CypherMode::{AES, CC20};

use dkcrypto::dk_crypto::DkEncrypt;

const phrases: [&str; 21] = [
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
    "I'm grateful for the opportunities I have.",
];

pub fn d10_performance_CC20() {
    for phrase in phrases.iter() {
        let _encrypted = DkEncrypt::new(CC20).encrypt_str(phrase, KEY).unwrap();
    }
}

pub fn d10_performance_AES128() {
    for phrase in phrases.iter() {
        let _encrypted = DkEncrypt::new(AES).encrypt_str(phrase, KEY).unwrap();
    }
}

const KEY: &str = "fqYVyce-Nh0HwpPQ7ZGZLog5s7PBLnwFMAW2OMnNPUs";

fn criterion_benchmark_1(c: &mut Criterion) {
    let mut c = c.benchmark_group("encrypt");
    c.sample_size(10);
    c.bench_function(BenchmarkId::new("encrypt 1", ""), |b| {
        b.iter(|| d10_performance_CC20())
    });
    c.finish();
}

fn criterion_benchmark_2(c: &mut Criterion) {
    let mut c = c.benchmark_group("encrypt");
    c.sample_size(10);
    c.bench_function(BenchmarkId::new("encrypt 2", ""), |b| {
        b.iter(|| d10_performance_AES128())
    });
    c.finish();
}

criterion_group!(benches, criterion_benchmark_1, criterion_benchmark_2);
criterion_main!(benches);

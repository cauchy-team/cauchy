use cauchy_consensus::*;
use criterion::*;
use rand::prelude::*;
use std::collections::HashMap;
use num_bigint::BigUint;

fn random() -> Entry {
    let mut rng = rand::thread_rng();
    let mut oddsketch = vec![0; ODDSKETCH_LEN];
    for i in 0..oddsketch.len() {
        oddsketch[i] = rng.gen();
    }
    let mass: u8 = rng.gen();
    Entry {
        oddsketch,
        mass: BigUint::from(mass),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("calc winner");
    for n in (64..(128 + 1)).step_by(32) {
        group.throughput(Throughput::Elements(n as u64));
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            let entry = random();
            entries.push(entry);
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| calculate_winner(black_box(&entries)))
        });
    }
    group.finish();
}

fn criterion_benchmark_par(c: &mut Criterion) {
    let mut group = c.benchmark_group("calc winner par");
    for n in (64..(128 + 1)).step_by(32) {
        group.throughput(Throughput::Elements(n as u64));
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            let entry = random();
            entries.push(entry);
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| calculate_winner_par(black_box(&entries)))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark, criterion_benchmark_par);
criterion_main!(benches);

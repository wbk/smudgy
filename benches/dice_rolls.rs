use criterion::{black_box, criterion_group, criterion_main, Criterion};

use smudgy::dice;

fn benchmark(c: &mut Criterion) {
    c.bench_function("roll two dice", |b| {
        b.iter(|| dice::dice_roll(black_box(2), black_box(6)))
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

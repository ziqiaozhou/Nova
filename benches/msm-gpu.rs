use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ff::{Field, PrimeField};
use halo2curves::{group::Group, CurveAffine};
use nova_snark::provider;
use nova_snark::provider::bn256_grumpkin::bn256;
use nova_snark::provider::pasta::vesta;
use provider::{msm, msm_gpu};
use rand_core::OsRng;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::time::Duration;

// cargo criterion --bench --no-default-features --features gpu --bench msm-gpu
// ---------------------------------------------------------
// Criterion setup
// ---------------------------------------------------------
criterion_group! {
  name = msm_gpu_group;
  config = Criterion::default().warm_up_time(Duration::from_secs(3));
  targets = msm_benchmarks
}
criterion_main!(msm_gpu_group);

// ---------------------------------------------------------
// Data preparation
// ---------------------------------------------------------
fn prepare_data<F: PrimeField, A: CurveAffine<ScalarExt = F>>(
  n: usize,
  bit_width: u32,
) -> (Vec<A>, Vec<u64>) {
  let bases: Vec<A> = (0..n)
    .into_par_iter()
    .map_init(
      || OsRng,
      |rng, _| {
        let scalar = F::random(rng);
        A::from(A::Curve::generator() * scalar)
      },
    )
    .collect();

  let mask = (1u64 << bit_width) - 1;
  let coeffs: Vec<u64> = (0..n)
    .into_par_iter()
    .map_init(|| OsRng, |_, _| rand::random::<u64>() & mask)
    .collect();

  (bases, coeffs)
}

// ---------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------
fn msm_benchmark<F: PrimeField, A: CurveAffine<ScalarExt = F>>(name: &str, c: &mut Criterion) {
  let sizes = [
    1024,
    1024 * 16,
    1024 * 32,
    1024 * 64,
    1024 * 128,
    1024 * 1024,
    4 * 1024 * 1024,
    16 * 1024 * 1024,
  ];
  let bit_width = 1;

  let mut group = c.benchmark_group(format!("MSM-{}", name));

  for &n in &sizes {
    println!("Preparing data for {} elements...", n);
    let (bases, coeffs) = prepare_data::<F, A>(n, bit_width);

    // CPU benchmark
    group.bench_with_input(
      BenchmarkId::new(format!("{}-CPU", name), n),
      &n,
      |b, &_size| {
        b.iter(|| {
          let _ = msm::msm_small(&coeffs, &bases);
        });
      },
    );

    // GPU benchmark
    gpu_host::cuda_ctx(0, |ctx, m| {
      let half_len = (bases.len() + 1) / 2;
      const MAX_BLOCK_DIM: u32 = 256;
      let block_dim = (half_len as u32).min(MAX_BLOCK_DIM);
      let grid_size = (half_len as u32 + block_dim - 1) / block_dim;

      // Allocate GPU buffers
      let d_bases = ctx.new_tensor_view(bases.as_slice()).unwrap();
      let mut d_partial_sums = ctx
        .new_tensor_view(vec![A::Curve::identity(); (grid_size * 2) as usize].as_slice())
        .unwrap();
      let mut d_scalars = ctx.new_tensor_view(coeffs.as_slice()).unwrap();

      group.bench_with_input(
        BenchmarkId::new(format!("{}-GPU", name), n),
        &n,
        |b, &_size| {
          b.iter(|| {
            d_scalars.copy_from_host(coeffs.as_slice()).unwrap();
            let _ = msm_gpu::msm_binary_gpu(ctx, m, &d_scalars, &d_bases, &mut d_partial_sums);
          });
        },
      );
    });
  }

  group.finish();
}

// ---------------------------------------------------------
// Entry point for Criterion
// ---------------------------------------------------------
fn msm_benchmarks(c: &mut Criterion) {
  msm_benchmark::<bn256::Scalar, bn256::Affine>("bn256", c);
  msm_benchmark::<vesta::Scalar, vesta::Affine>("vesta", c);
}

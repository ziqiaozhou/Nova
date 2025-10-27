use ff::{Field, PrimeField};
use halo2curves::{group::Group, CurveAffine};
use nova_snark::provider;
use nova_snark::provider::bn256_grumpkin::bn256;
use rand_core::OsRng;

fn run<F: PrimeField, A: CurveAffine<ScalarExt = F>>() {
  let n = 1024 * 16;
  let bases = (0..n)
    .map(|_| A::from(A::generator() * F::random(OsRng)))
    .collect::<Vec<_>>();
  let bit_width = 1;
  let coeffs: Vec<u64> = (0..n)
    .map(|_| rand::random::<u64>() % (1 << bit_width))
    .collect::<Vec<_>>();
  for _ in 0..std::env::var("MSM_BINARY_GPU_ITER").unwrap_or("1".to_string()).parse::<u32>().unwrap_or(1) {
    let _ = provider::msm::msm_small(&coeffs, &bases);
  }
}

fn main() {
  run::<bn256::Scalar, bn256::Affine>();
}

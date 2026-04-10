//! Shared helpers for sparse materialization examples.
//!
//! This module is intentionally storage-agnostic. It randomizes chunk ordering,
//! drives materialization, and reports generic progress while delegating
//! backend-specific inspection to caller-provided callbacks.

use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use clap::Args;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::common::sparse_fill_visualizer::render_sparse_fill_bar;

#[derive(Args, Debug, Clone)]
/// CLI options shared by sparse materialization examples.
pub struct SparseMaterializationOptions {
    /// Chunk size to use for sparse materialization.
    #[arg(long, default_value_t = 262144)]
    pub chunk_size: usize,

    /// Percentage of chunks to materialize (0 < percent <= 100).
    #[arg(long, default_value_t = 100.0)]
    pub fill_percent: f64,

    /// Sleep between chunk materialization steps (milliseconds).
    #[arg(long, default_value_t = 0)]
    pub sleep_ms: u64,

    /// Width of the ASCII progress bar.
    #[arg(long, default_value_t = 32)]
    pub progress_width: usize,
}

impl SparseMaterializationOptions {
    /// Validates CLI options before materialization begins.
    pub fn validate(&self) -> std::io::Result<()> {
        if self.chunk_size == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--chunk-size must be greater than zero",
            ));
        }
        if !(self.fill_percent > 0.0 && self.fill_percent <= 100.0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--fill-percent must be in the range (0, 100]",
            ));
        }
        if self.progress_width == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--progress-width must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Runtime inputs for a sparse materialization run.
pub struct SparseMaterializationConfig<'a> {
    /// Total logical object length in bytes.
    pub len: usize,
    /// Materialization behavior flags.
    pub options: &'a SparseMaterializationOptions,
}

/// Outputs captured during sparse materialization.
pub struct SparseMaterializationResult {
    /// Chunk-aligned logical offsets for the object.
    pub logical_offsets: Vec<usize>,
    /// Offsets that were materialized during this run.
    pub filled_offsets: HashSet<usize>,
}

/// Per-step snapshot delivered to caller-provided callbacks.
pub struct SparseMaterializationStep<'a> {
    /// Zero-based step index.
    pub index: usize,
    /// Total number of selected materialization steps.
    pub total_steps: usize,
    /// Randomized read point requested from the backend.
    pub requested_offset: usize,
    /// Chunk-aligned offset corresponding to `requested_offset`.
    pub normalized_offset: usize,
    /// Bytes materialized by this step.
    pub chunk_len: usize,
    /// Running total of materialized bytes.
    pub materialized_bytes: usize,
    /// Materialization completion percentage over selected steps.
    pub progress_percent: f64,
    /// Total logical object length in bytes.
    pub len: usize,
    /// Chunk size in bytes.
    pub chunk_size: usize,
    /// All chunk-aligned offsets for the object.
    pub logical_offsets: &'a [usize],
    /// Set of chunk-aligned offsets that have been materialized.
    pub filled_offsets: &'a HashSet<usize>,
}

/// Materializes randomized chunk offsets by invoking `materialize_chunk`.
///
/// `after_chunk` is called after each successful chunk materialization so each
/// example can compute backend-specific diagnostics.
pub async fn run_sparse_materialization<F, Fut, G>(
    config: SparseMaterializationConfig<'_>,
    mut materialize_chunk: F,
    mut after_chunk: G,
) -> std::io::Result<SparseMaterializationResult>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = std::io::Result<usize>>,
    G: FnMut(SparseMaterializationStep<'_>) -> std::io::Result<()>,
{
    config.options.validate()?;
    let len = config.len;
    let chunk_size = config.options.chunk_size;

    let logical_offsets: Vec<usize> = (0..len).step_by(chunk_size).collect();
    let mut rng = rand::thread_rng();
    let mut randomized_offsets = logical_offsets.clone();
    shuffle_offsets(&mut randomized_offsets, &mut rng);
    let offsets_all = jittered_offsets(&randomized_offsets, chunk_size, len, &mut rng);
    let fill_chunks = ((offsets_all.len() as f64) * (config.options.fill_percent / 100.0)).ceil() as usize;
    let fill_chunks = fill_chunks.max(1).min(offsets_all.len());
    let offsets: Vec<usize> = offsets_all.into_iter().take(fill_chunks).collect();

    println!(
        "materializing {:.2}% of {} bytes: {} / {} chunk(s), random order",
        config.options.fill_percent,
        len,
        offsets.len(),
        logical_offsets.len()
    );
    println!("randomized read points (selected): {:?}", offsets);
    println!(
        "sparse map width={} sleep={}ms between steps",
        config.options.progress_width, config.options.sleep_ms
    );

    let mut materialized_bytes = 0usize;
    let mut filled_offsets = HashSet::new();

    for (index, offset) in offsets.iter().copied().enumerate() {
        let chunk_len = materialize_chunk(offset).await?;
        let normalized_offset = offset - (offset % chunk_size);
        filled_offsets.insert(normalized_offset);
        materialized_bytes += chunk_len;

        let progress_step = index + 1;
        let progress_percent = (progress_step as f64 * 100.0) / offsets.len() as f64;
        let sparse_fill_bar = render_sparse_fill_bar(&filled_offsets, &logical_offsets, config.options.progress_width);

        println!(
            "filled chunk {} from requested offset {} -> normalized {} ({} bytes)",
            index, offset, normalized_offset, chunk_len
        );
        println!(
            "sparse fill map [{}] {:>6.2}% ({}/{})",
            sparse_fill_bar,
            progress_percent,
            filled_offsets.len(),
            logical_offsets.len()
        );
        println!("materialized payload: {} / {} bytes", materialized_bytes, len);

        after_chunk(SparseMaterializationStep {
            index,
            total_steps: offsets.len(),
            requested_offset: offset,
            normalized_offset,
            chunk_len,
            materialized_bytes,
            progress_percent,
            len,
            chunk_size,
            logical_offsets: &logical_offsets,
            filled_offsets: &filled_offsets,
        })?;

        if config.options.sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(config.options.sleep_ms));
        }
    }

    Ok(SparseMaterializationResult {
        logical_offsets,
        filled_offsets,
    })
}

/// Adds per-offset random intra-chunk jitter while preserving chunk coverage to simulate more realistic access
/// patterns and validate overlap dedupe.
fn jittered_offsets<R: Rng + ?Sized>(offsets: &[usize], chunk_size: usize, len: usize, rng: &mut R) -> Vec<usize> {
    offsets
        .iter()
        .map(|offset| {
            let remaining = len.saturating_sub(*offset);
            let max_jitter = remaining.min(chunk_size).saturating_sub(1);
            let jitter = if max_jitter == 0 {
                0
            } else {
                rng.gen_range(0..=max_jitter)
            };

            offset + jitter
        })
        .collect()
}

/// Randomizes chunk ordering in-place.
fn shuffle_offsets<R: Rng + ?Sized>(offsets: &mut [usize], rng: &mut R) {
    if offsets.len() <= 1 {
        return;
    }
    offsets.shuffle(rng);
}

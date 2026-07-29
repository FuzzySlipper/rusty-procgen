# Rusty Engine CA scale baseline

Status: checked first baseline for `fd9ff4733142e5de76813af217341bcaf50a187e` against Rusty Engine `db5641fc4e9d033112bc2b374a35933c3838e39c`.

## Reproduce and validate

```bash
pnpm run engine:ca:scale
pnpm run engine:ca:scale:check
```

The first command regenerates the release benchmark, runs its real Chromium
consumer, and rewrites the versioned matrix plus this report. The check command
recomputes deterministic summaries and source hashes without treating timings
as equality gates.

## Matrix

| Scenario | Domain cells | Authority voxels initial/peak | Peak CA density | Changed median/max | Resident chunks/max quads | Median measured step | Changed cells/s | Trace bytes | Browser apply/two-frame |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sparse-propagation | 243 | 1/183 | 75.31% | 56/92 | 3/246 | 333.339 µs | 157,427 | 137.14 KiB | 6.200 / 62.500 ms |
| dense-churn | 108 | 1/55 | 50.93% | 107/107 | 1/324 | 448.849 µs | 232,647 | 188.21 KiB | 5.300 / 66.500 ms |
| cross-boundary | 405 | 1/189 | 46.67% | 54/128 | 8/390 | 727.010 µs | 74,277 | 160.75 KiB | 7.400 / 66.300 ms |
| large-resident-small-hot-region | 65536 | 65536/65536 | 0.20% | 40/104 | 128/12288 | 29.802 ms | 1,338 | 1.68 MiB | 118.600 / 369.300 ms |
| high-surface-area | 400 | 1/201 | 50.25% | 399/399 | 4/1200 | 1.761 ms | 224,291 | 688.54 KiB | 17.200 / 119.200 ms |

Rust uses 1 warmup and
2 recorded runs on
linux/x86_64 with
`rustc 1.96.0 (ac68faa20 2026-05-25) (Arch Linux rust 1:1.96.0-1)`. Structural hashes agree
across repeats. Browser values summarize three descriptive first-step
interaction samples per scenario on `Chromium 148.0.7778.215 Arch Linux`; they
are not thresholds.

## Findings

- Slowest median measured step:
  `large-resident-small-hot-region` at
  29.802 ms; its largest
  measured phase was
  `spatialPreviewNs`.
- Largest encoded trace:
  `large-resident-small-hot-region` at
  1.68 MiB.
- Largest resident scope:
  `large-resident-small-hot-region` at
  65,536
  authority voxels across
  128 resident chunks in a
  65,536-cell
  domain.
- Highest published mesh surface:
  `large-resident-small-hot-region` at
  12,288
  quads.
- No optimization task created: this first matrix exposes associations but does not isolate a production bottleneck from benchmark readback/encoding overhead.

These are workload associations, not single-factor causal conclusions. The
matrix distinguishes latency, changed-cell throughput, encoded transfer size,
resident scope, update density, and mesh surface. Browser playback proves that
sparse and stress traces remain interactive, but visual smoothness is
**not measured** because the bounded 4–6-step traces do not include a
frame-pacing sampler.

## Nonclaims

- Timing samples are observations from the declared hosts, never equality gates.
- The five fixtures vary multiple dimensions and do not establish single-factor causality.
- No memory allocation, dirty-region, GPU utilization, transfer-network, or frame-pacing measurement was taken.
- Browser presentation time is interaction-to-two-animation-frames, not GPU completion time.
- This bounded matrix is neither an Engine scale ceiling nor a gameplay runtime benchmark.

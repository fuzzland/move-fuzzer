use std::time::Instant;

/// Size of coverage map segments for statistics reporting
const COVERAGE_SEGMENT_SIZE: usize = 4096;

// Print fuzzer statistics with coverage breakdown
pub fn print_fuzzer_stats(
    start_time: Instant,
    executions: u64,
    corpus_size: usize,
    solutions_size: usize,
    coverage_map: &[u8],
    total_instructions_executed: u64,
    total_possible_edges: usize,
) {
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();

    let exec_per_sec = if elapsed_secs > 0.0 {
        executions as f64 / elapsed_secs
    } else {
        0.0
    };

    let covered_edges = coverage_map.iter().filter(|&&b| b > 0).count();

    let (edge_display, edge_coverage_pct) = if total_possible_edges > 0 {
        let pct = (covered_edges as f64 / total_possible_edges as f64) * 100.0;
        (format!("{}/{}", covered_edges, total_possible_edges), pct)
    } else {
        (format!("{} discovered", covered_edges), 0.0)
    };

    let exec_per_sec_str = if exec_per_sec >= 1000.0 {
        format!("{:.3}k", exec_per_sec / 1000.0)
    } else {
        format!("{:.0}", exec_per_sec)
    };

    if total_possible_edges > 0 {
        println!(
            "run time: {:.0}s, clients: 1, corpus: {}, objectives: {}, executions: {}, exec/sec: {}, edges: {} ({:.2}%)",
            elapsed_secs,
            corpus_size,
            solutions_size,
            executions,
            exec_per_sec_str,
            edge_display,
            edge_coverage_pct
        );
    } else {
        println!(
            "run time: {:.0}s, clients: 1, corpus: {}, objectives: {}, executions: {}, exec/sec: {}, edges: {}",
            elapsed_secs, corpus_size, solutions_size, executions, exec_per_sec_str, edge_display
        );
    }

    // Print compact coverage summary
    let avg_instrs = if executions > 0 {
        total_instructions_executed as f64 / executions as f64
    } else {
        0.0
    };

    let covered_segments = count_covered_segments(coverage_map, COVERAGE_SEGMENT_SIZE);
    let total_segments = coverage_map.len().div_ceil(COVERAGE_SEGMENT_SIZE);

    println!(
        "instrs: {} (avg {:.1}/exec), segments: {}/{}",
        total_instructions_executed, avg_instrs, covered_segments, total_segments
    );
}

// Count segments that have any coverage
fn count_covered_segments(coverage_map: &[u8], segment_size: usize) -> usize {
    let num_segments = coverage_map.len().div_ceil(segment_size);
    let mut covered = 0;

    for seg_idx in 0..num_segments {
        let start = seg_idx * segment_size;
        let end = ((seg_idx + 1) * segment_size).min(coverage_map.len());
        if coverage_map[start..end].iter().any(|&b| b > 0) {
            covered += 1;
        }
    }

    covered
}

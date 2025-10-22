use std::time::Instant;

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
            elapsed_secs,
            corpus_size,
            solutions_size,
            executions,
            exec_per_sec_str,
            edge_display
        );
    }
    
    println!(
        "Total instructions executed: {} (avg {:.1} per execution)",
        total_instructions_executed,
        if executions > 0 { total_instructions_executed as f64 / executions as f64 } else { 0.0 }
    );
    
    print_coverage_segments(coverage_map);
}

// Print coverage breakdown by segments
fn print_coverage_segments(coverage_map: &[u8]) {
    const SEGMENT_SIZE: usize = 4096;
    let total_edges = coverage_map.len();
    let num_segments = total_edges.div_ceil(SEGMENT_SIZE);
    
    print!("Coverage by segment: ");
    for seg_idx in 0..num_segments {
        let start = seg_idx * SEGMENT_SIZE;
        let end = ((seg_idx + 1) * SEGMENT_SIZE).min(total_edges);
        let segment = &coverage_map[start..end];
        
        let seg_covered = segment.iter().filter(|&&b| b > 0).count();
        let seg_total = segment.len();
        let seg_pct = (seg_covered as f64 / seg_total as f64) * 100.0;
        
        let display = format_segment_coverage(seg_idx, seg_pct);
        print!("{} ", display);
        
        if (seg_idx + 1) % 8 == 0 && seg_idx + 1 < num_segments {
            print!("\n                     ");
        }
    }
    println!();
}

// Format segment coverage display
fn format_segment_coverage(seg_idx: usize, seg_pct: f64) -> String {
    if seg_pct >= 99.9 {
        format!("[{:2}:✓]", seg_idx)
    } else if seg_pct > 0.0 {
        format!("[{:2}:{:2.0}%]", seg_idx, seg_pct)
    } else {
        format!("[{:2}:--]", seg_idx)
    }
}

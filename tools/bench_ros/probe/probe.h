#pragma once
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
enum { R2R_BENCH_PROBE_ABI = 1 };
typedef struct r2r_bench_probe_stats {
  uint64_t malloc_calls, calloc_calls, realloc_calls, free_calls, null_free_calls;
  uint64_t aligned_calls, failed_allocations, requested_bytes, size_overflows, counter_overflows;
  uint64_t user_cpu_ns, system_cpu_ns, voluntary_context_switches, involuntary_context_switches;
  uint64_t process_max_rss_kib;
} r2r_bench_probe_stats;
int r2r_bench_probe_version(void);
int r2r_bench_probe_begin(void);
int r2r_bench_probe_end(r2r_bench_probe_stats *stats);
#ifdef R2R_BENCH_PROBE_TESTING
void r2r_bench_probe_test_pause(void);
int r2r_bench_probe_test_observed(void);
void r2r_bench_probe_test_resume(void);
#endif
#ifdef __cplusplus
}
#endif

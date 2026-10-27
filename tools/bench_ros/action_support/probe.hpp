#pragma once
#include "../probe/probe.h"
#if defined(__linux__)
#include <dlfcn.h>
#endif
#include <optional>
#include <ostream>
#include <stdexcept>

namespace action_bench {
struct ProbeFrame {
  r2r_bench_probe_stats native{};
  uint64_t polls=0, spins=0;
  void render(std::ostream &out) const {
    out << "{\"abi\":1,\"allocations\":{\"malloc_calls\":" << native.malloc_calls
      << ",\"calloc_calls\":" << native.calloc_calls << ",\"realloc_calls\":" << native.realloc_calls
      << ",\"free_calls\":" << native.free_calls << ",\"null_free_calls\":" << native.null_free_calls
      << ",\"aligned_calls\":" << native.aligned_calls << ",\"failed_allocations\":" << native.failed_allocations
      << ",\"requested_bytes\":" << native.requested_bytes << ",\"size_overflows\":" << native.size_overflows
      << ",\"counter_overflows\":" << native.counter_overflows
      << "},\"resources\":{\"user_cpu_ns\":" << native.user_cpu_ns << ",\"system_cpu_ns\":" << native.system_cpu_ns
      << ",\"voluntary_context_switches\":" << native.voluntary_context_switches
      << ",\"involuntary_context_switches\":" << native.involuntary_context_switches
      << ",\"process_max_rss_kib\":" << native.process_max_rss_kib
      << "},\"application\":{\"future_polls\":" << polls << ",\"spin_calls\":" << spins << ",\"future_wakes\":null}}";
  }
};
class Driver {
  using Version=int (*)();
  using Begin=int (*)();
  using End=int (*)(r2r_bench_probe_stats *);
  Begin begin_=nullptr;
  End end_=nullptr;
  bool active_=false;
  uint64_t polls_=0,spins_=0;
public:
  Driver() {
#if defined(__linux__)
    static_assert(sizeof(r2r_bench_probe_stats)==120);
    auto version=reinterpret_cast<Version>(dlsym(RTLD_DEFAULT,"r2r_bench_probe_version"));
    if (!version) return;
    begin_=reinterpret_cast<Begin>(dlsym(RTLD_DEFAULT,"r2r_bench_probe_begin"));
    end_=reinterpret_cast<End>(dlsym(RTLD_DEFAULT,"r2r_bench_probe_end"));
    if (version()!=1 || !begin_ || !end_) throw std::runtime_error("incompatible benchmark probe ABI");
#endif
  }
  void begin() {
    if (!begin_) return;
    polls_=spins_=0;
    if (begin_()) throw std::runtime_error("cannot begin benchmark probe frame");
    active_=true;
  }
  std::optional<ProbeFrame> end() {
    if (!active_) return std::nullopt;
    active_=false;
    ProbeFrame frame;
    if (end_(&frame.native)) throw std::runtime_error("cannot finish benchmark probe frame");
    frame.polls=polls_;frame.spins=spins_;
    return frame;
  }
  void poll() { if (active_) ++polls_; }
  template<class Executor,class Duration> void spin(Executor &executor,Duration duration) {
    if (active_) ++spins_;
    executor.spin_once(duration);
  }
  ~Driver() { if (active_) { r2r_bench_probe_stats ignored; end_(&ignored); } }
};
}

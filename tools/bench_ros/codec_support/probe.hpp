#pragma once
#include "../probe/probe.h"
#include <stdexcept>
#ifdef __linux__
#include <dlfcn.h>
#endif

struct CodecProbe {
  int (*begin)() = nullptr;
  int (*end)(r2r_bench_probe_stats *) = nullptr;
  CodecProbe() {
#ifdef __linux__
    auto version = reinterpret_cast<int (*)()>(dlsym(RTLD_DEFAULT, "r2r_bench_probe_version"));
    if (!version) return;
    begin = reinterpret_cast<int (*)()>(dlsym(RTLD_DEFAULT, "r2r_bench_probe_begin"));
    end = reinterpret_cast<int (*)(r2r_bench_probe_stats *)>(dlsym(RTLD_DEFAULT, "r2r_bench_probe_end"));
    if (version() != 1 || !begin || !end || sizeof(r2r_bench_probe_stats) != 120)
      throw std::runtime_error("incompatible codec allocation probe ABI");
#endif
  }
};

#pragma once
#include <atomic>
#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <mutex>
#include <stdexcept>
#include <vector>
#include <fstream>
#include <string>
#include <time.h>

namespace group_bench {
using Clock=std::chrono::steady_clock;
__attribute__((noinline)) inline int64_t work(size_t sequence,size_t operations) {
  uint64_t value=sequence+UINT64_C(0x9e3779b97f4a7c15);
  for(size_t step=0;step<operations;++step) {
    value^=value<<13;value^=value>>7;value^=value<<17;
    value+=step+UINT64_C(0x517cc1b727220a95);
  }
  return static_cast<int64_t>(value&INT64_MAX);
}
struct Row {
  std::atomic<size_t> seen{0};
  std::atomic<int64_t> native{0};
  std::atomic<uint64_t> enter{0},finish{0};
};
struct State {
  Clock::time_point origin=Clock::now();
  std::vector<Row> rows;
  std::atomic<size_t> active{0},peak{0},probes{0};
  std::mutex mutex;
  std::condition_variable ready;
  bool released=false;
  size_t limit,operations;
  State(size_t count,size_t limit_,size_t operations_):rows(count),limit(limit_),operations(operations_){}
  uint64_t now() const {return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now()-origin).count();}
  void release() { {std::lock_guard<std::mutex> lock(mutex);released=true;} ready.notify_all(); }
  int64_t invoke(int64_t a,int64_t b,int64_t native) {
    const auto running=active.fetch_add(1,std::memory_order_acq_rel)+1;
    struct Retire {State *state;~Retire(){state->active.fetch_sub(1,std::memory_order_release);}} retire{this};
    if(running>limit)throw std::runtime_error("callback group concurrency bound exceeded");
    auto old=peak.load(std::memory_order_relaxed);while(old<running&&!peak.compare_exchange_weak(old,running,std::memory_order_relaxed)){}
    if(a<0) {
      if((a!=-3&&a!=-4)||b)throw std::runtime_error("invalid overlap challenge");
      if(probes.fetch_add(1,std::memory_order_acq_rel)==1)release();
      std::unique_lock<std::mutex> lock(mutex);
      if(!ready.wait_for(lock,std::chrono::seconds(5),[&]{return released;}))throw std::runtime_error("same-source overlap challenge timeout");
      return a;
    }
    if(b!=static_cast<int64_t>(operations)||static_cast<uint64_t>(a)>=rows.size())throw std::runtime_error("request dimensions mismatch");
    auto &row=rows[a];
    if(row.seen.exchange(1,std::memory_order_acq_rel))throw std::runtime_error("duplicate request");
    row.native.store(native,std::memory_order_relaxed);row.enter.store(now(),std::memory_order_release);
    auto result=work(a,operations);row.finish.store(now(),std::memory_order_release);return result;
  }
};
struct ProcessMetrics {size_t threads=0;uint64_t rss=0,hwm=0;};
inline ProcessMetrics process_metrics() {
  std::ifstream input("/proc/self/status");std::string key,line;ProcessMetrics value;
  while(input>>key) {
    if(key=="Threads:")input>>value.threads;
    else if(key=="VmRSS:")input>>value.rss;
    else if(key=="VmHWM:")input>>value.hwm;
    std::getline(input,line);
  }
  if(!value.threads||!value.rss||!value.hwm)throw std::runtime_error("Linux process metrics missing");
  value.hwm=std::max(value.hwm,value.rss);return value;
}
inline uint64_t cpu_ns() {
  timespec value{};if(clock_gettime(CLOCK_PROCESS_CPUTIME_ID,&value))throw std::runtime_error("CPU clock failed");
  return static_cast<uint64_t>(value.tv_sec)*1000000000+value.tv_nsec;
}
}

#pragma once
#include <algorithm>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
#include "probe.hpp"

namespace action_bench {
struct Report {
  uint64_t checksum = 0, elapsed = 0;
  std::vector<uint64_t> samples, acceptance, cancellation;
  std::optional<ProbeFrame> probe;
};
inline uint64_t percentile(std::vector<uint64_t> values, size_t percent) {
  std::sort(values.begin(), values.end());
  return values[((values.size() - 1) * percent + 99) / 100];
}
inline void array_json(const std::vector<uint64_t> &values) {
  std::cout << '[';
  for (size_t i = 0; i < values.size(); ++i) {
    if (i) std::cout << ',';
    std::cout << values[i];
  }
  std::cout << ']';
}
inline void render(const std::vector<Report> &reports, const std::string &role,
                   const char *rmw, size_t scenario, size_t iterations, size_t warmup, uint64_t process_elapsed) {
  const size_t count = reports.size(), total = (iterations + warmup) * count;
  Report combined;
  for (const auto &report : reports) {
    combined.checksum += report.checksum;
    combined.elapsed += report.elapsed;
    combined.samples.insert(combined.samples.end(), report.samples.begin(), report.samples.end());
    combined.acceptance.insert(combined.acceptance.end(), report.acceptance.begin(), report.acceptance.end());
    combined.cancellation.insert(combined.cancellation.end(), report.cancellation.begin(), report.cancellation.end());
  }
  std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"" << role << "\",\"rmw\":\"" << rmw
    << "\",\"scenario\":" << scenario << ",\"iterations\":" << iterations * count << ",\"warmup\":" << warmup * count
    << ",\"received\":" << total << ",\"checksum\":" << combined.checksum << ",\"accepted\":" << total
    << ",\"feedback\":" << total << ",\"results\":" << total << ",\"canceled\":" << (scenario ? total : 0);
  if (role == "ping") {
    std::cout << ",\"elapsed_ns\":" << combined.elapsed << ",\"roundtrips_per_second\":" << iterations * count / (combined.elapsed * 1e-9)
      << ",\"rtt_ns_p50\":" << percentile(combined.samples,50) << ",\"rtt_ns_p95\":" << percentile(combined.samples,95)
      << ",\"rtt_ns_p99\":" << percentile(combined.samples,99) << ",\"rtt_ns_max\":" << *std::max_element(combined.samples.begin(),combined.samples.end())
      << ",\"samples_ns\":";
    array_json(combined.samples);
    std::cout << ",\"stage_samples_ns\":{\"acceptance\":";
    array_json(combined.acceptance);
    std::cout << ",\"cancellation\":";
    array_json(combined.cancellation);
    std::cout << "},\"acceptance_ns_p50\":" << percentile(combined.acceptance,50) << ",\"cancellation_ns_p50\":";
    if (scenario) std::cout << percentile(combined.cancellation,50); else std::cout << "null";
  }
  if (count==1 && reports[0].probe) { std::cout << ",\"probe\":"; reports[0].probe->render(std::cout); }
  if (count > 1) {
    std::cout << ",\"cohort_process_elapsed_ns\":" << process_elapsed;
    std::cout << ",\"cohort_config\":{\"count\":" << count << ",\"iterations\":" << iterations
      << ",\"warmup\":" << warmup << ",\"maximum_retained_goals\":" << iterations+warmup
      << ",\"fresh_endpoints\":true},\"cohorts\":[";
    for (size_t i=0; i<count; ++i) {
      if (i) std::cout << ',';
      std::cout << "{\"index\":" << i << ",\"sequence_offset\":" << i*(iterations+warmup)
        << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << iterations+warmup << ",\"accepted\":" << iterations+warmup
        << ",\"feedback\":" << iterations+warmup << ",\"results\":" << iterations+warmup
        << ",\"canceled\":" << (scenario ? iterations+warmup : 0) << ",\"checksum\":" << reports[i].checksum;
      if (role == "ping") std::cout << ",\"elapsed_ns\":" << reports[i].elapsed;
      if (reports[i].probe) { std::cout << ",\"probe\":"; reports[i].probe->render(std::cout); }
      std::cout << '}';
    }
    std::cout << ']';
  }
  std::cout << "}\n";
}
}

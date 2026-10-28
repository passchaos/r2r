#pragma once
#include "probe.hpp"
#include <algorithm>
#include <chrono>
#include <cstring>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

#include <rclcpp/serialization.hpp>
#include <rclcpp/serialized_message.hpp>
#include <rmw/rmw.h>

template<class T> void escape(const T &value) {
  asm volatile("" : : "g"(&value) : "memory");
}

uint64_t fingerprint(const uint8_t *data, size_t size) {
  uint64_t result = 0xcbf29ce484222325;
  for (size_t i = 0; i < size; ++i) result = (result ^ data[i]) * 0x100000001b3;
  return result;
}

template<class F> int run_codec(int argc, char **argv) {
  try {
    if (argc < 4 || argc > 5) throw std::invalid_argument("usage: rclcpp_comparison <copy_reuse|serialize_owned|serialize_reuse|serialize_owned_reuse|deserialize_owned> <elements> <iterations> [wire-file]");
    const std::string mode = argv[1];
    const size_t elements = std::stoull(argv[2]), iterations = std::stoull(argv[3]);
    if (iterations == 0 || elements > UINT32_MAX) throw std::invalid_argument("invalid benchmark dimensions");
    using Message = typename F::Message;
    Message source = F::message(elements);
    rclcpp::Serialization<Message> serializer;
    rclcpp::SerializedMessage input;
    serializer.serialize_message(&source, &input);
    Message decoded;
    serializer.deserialize_message(&input, &decoded);
    if (decoded != source) throw std::runtime_error("full deserialization oracle failed");
    const auto &wire = input.get_rcl_serialized_message();
    if (argc == 5) {
      std::ofstream file(argv[4], std::ios::binary);
      file.write(reinterpret_cast<const char *>(wire.buffer), static_cast<std::streamsize>(wire.buffer_length));
      if (!file) throw std::runtime_error("cannot write wire fixture");
    }
    Message destination = source;
    rclcpp::SerializedMessage serialized_scratch(input.size());
    auto pointers = F::pointers(destination);
    size_t pointer_changes = 0;
    uint64_t checksum = 0;
    auto operation = [&]() -> uint64_t {
      if (mode == "copy_reuse") {
        escape(source); escape(destination);
        destination = source;
        if (F::pointers(destination) != pointers) ++pointer_changes;
        escape(destination);
        return F::tag(destination);
      }
      if (mode == "serialize_owned" || mode == "serialize_owned_reuse" || mode == "serialize_reuse") {
        escape(source);
        serializer.serialize_message(&source, &serialized_scratch);
        const auto &serialized = serialized_scratch.get_rcl_serialized_message();
        if (mode == "serialize_reuse") {
          escape(serialized);
          return serialized.buffer_length ^ serialized.buffer[serialized.buffer_length - 1];
        }
        std::vector<uint8_t> output(serialized.buffer, serialized.buffer + serialized.buffer_length);
        escape(output);
        return output.size() ^ output.back();
      }
      if (mode == "deserialize_owned") {
        escape(input);
        Message output;
        serializer.deserialize_message(&input, &output);
        escape(output);
        return F::owned_tag(output);
      }
      throw std::invalid_argument("unknown benchmark mode");
    };
    for (size_t i = 0; i < 100; ++i) { auto value = operation(); escape(value); }
    std::vector<double> batches;
    batches.reserve((iterations + 999) / 1000);
    CodecProbe probe;
    if (probe.begin && probe.begin()) throw std::runtime_error("cannot start codec allocation frame");
    const auto start = std::chrono::steady_clock::now();
    for (size_t completed = 0; completed < iterations;) {
      const size_t count = std::min<size_t>(1000, iterations - completed);
      const auto batch = std::chrono::steady_clock::now();
      for (size_t i = 0; i < count; ++i) checksum += operation();
      const auto ns = std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - batch).count();
      batches.push_back(static_cast<double>(ns) / count);
      completed += count;
    }
    const auto ns = std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - start).count();
    r2r_bench_probe_stats stats{};
    if (probe.end && probe.end(&stats)) throw std::runtime_error("cannot stop codec allocation frame");
    if (mode == "copy_reuse" && destination != source) throw std::runtime_error("full copy oracle failed");
    if ((mode == "serialize_reuse" || mode == "serialize_owned_reuse") &&
        (serialized_scratch.size() != input.size() || std::memcmp(serialized_scratch.get_rcl_serialized_message().buffer, wire.buffer, wire.buffer_length)))
      throw std::runtime_error("full serialized output oracle failed");
    std::sort(batches.begin(), batches.end());
    auto percentile = [&](size_t percent) { return batches[((batches.size() - 1) * percent + 99) / 100]; };
    std::cout << "{\"implementation\":\"rclcpp\",\"rmw\":\"" << rmw_get_implementation_identifier()
      << "\",\"fixture\":\"" << F::name << "\",\"mode\":\"" << mode << "\",\"elements\":" << elements
      << ",\"iterations\":" << iterations << ",\"elapsed_ns\":" << ns << ",\"ns_per_op\":" << static_cast<double>(ns) / iterations
      << ",\"batch_ns_per_op_p50\":" << percentile(50) << ",\"batch_ns_per_op_p95\":" << percentile(95)
      << ",\"batch_ns_per_op_p99\":" << percentile(99) << ",\"wire_bytes\":" << wire.buffer_length
      << ",\"wire_hash\":" << fingerprint(wire.buffer, wire.buffer_length) << ",\"checksum\":" << checksum
      << ",\"native_copy_pointer_changes\":" << pointer_changes << ",\"allocation_frame\":";
    if (!probe.end) std::cout << "null";
    else std::cout << "{\"malloc_calls\":" << stats.malloc_calls << ",\"calloc_calls\":" << stats.calloc_calls
      << ",\"realloc_calls\":" << stats.realloc_calls << ",\"free_calls\":" << stats.free_calls
      << ",\"aligned_calls\":" << stats.aligned_calls << ",\"failed_allocations\":" << stats.failed_allocations
      << ",\"requested_bytes\":" << stats.requested_bytes << ",\"size_overflows\":" << stats.size_overflows
      << ",\"counter_overflows\":" << stats.counter_overflows << "}";
    std::cout << "}\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}

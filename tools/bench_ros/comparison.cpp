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
#include <std_msgs/msg/int32_multi_array.hpp>
#include <rmw/rmw.h>

template<class T> void escape(const T &value) {
  asm volatile("" : : "g"(&value) : "memory");
}

uint64_t fingerprint(const uint8_t *data, size_t size) {
  uint64_t result = 0xcbf29ce484222325;
  for (size_t i = 0; i < size; ++i) result = (result ^ data[i]) * 0x100000001b3;
  return result;
}

int main(int argc, char **argv) {
  try {
    if (argc < 4 || argc > 5) throw std::invalid_argument("usage: rclcpp_comparison <copy_reuse|serialize_owned|deserialize_owned> <elements> <iterations> [wire-file]");
    const std::string mode = argv[1];
    const size_t elements = std::stoull(argv[2]), iterations = std::stoull(argv[3]);
    if (iterations == 0 || elements > UINT32_MAX) throw std::invalid_argument("invalid benchmark dimensions");
    std_msgs::msg::Int32MultiArray source;
    source.layout.dim.resize(1);
    source.layout.dim[0].label = "samples";
    source.layout.dim[0].size = static_cast<uint32_t>(elements);
    source.layout.dim[0].stride = static_cast<uint32_t>(elements);
    source.data.resize(elements);
    for (size_t i = 0; i < elements; ++i) source.data[i] = static_cast<int32_t>((i * 17 + 3) % 100003) - 50000;
    rclcpp::Serialization<std_msgs::msg::Int32MultiArray> serializer;
    rclcpp::SerializedMessage input;
    serializer.serialize_message(&source, &input);
    std_msgs::msg::Int32MultiArray decoded;
    serializer.deserialize_message(&input, &decoded);
    if (decoded != source) throw std::runtime_error("full deserialization oracle failed");
    const auto &wire = input.get_rcl_serialized_message();
    if (argc == 5) {
      std::ofstream file(argv[4], std::ios::binary);
      file.write(reinterpret_cast<const char *>(wire.buffer), static_cast<std::streamsize>(wire.buffer_length));
      if (!file) throw std::runtime_error("cannot write wire fixture");
    }
    std_msgs::msg::Int32MultiArray destination = source;
    rclcpp::SerializedMessage serialized_scratch(input.size());
    auto data_pointer = destination.data.data();
    auto dimension_pointer = destination.layout.dim.data();
    size_t pointer_changes = 0;
    uint64_t checksum = 0;
    auto operation = [&]() -> uint64_t {
      if (mode == "copy_reuse") {
        escape(source); escape(destination);
        destination = source;
        if (destination.data.data() != data_pointer || destination.layout.dim.data() != dimension_pointer) ++pointer_changes;
        escape(destination);
        return destination.data.size() ^ destination.layout.dim.size();
      }
      if (mode == "serialize_owned") {
        escape(source);
        serializer.serialize_message(&source, &serialized_scratch);
        const auto &serialized = serialized_scratch.get_rcl_serialized_message();
        std::vector<uint8_t> output(serialized.buffer, serialized.buffer + serialized.buffer_length);
        escape(output);
        return output.size() ^ output.back();
      }
      if (mode == "deserialize_owned") {
        escape(input);
        std_msgs::msg::Int32MultiArray output;
        serializer.deserialize_message(&input, &output);
        escape(output);
        return output.data.size() ^ static_cast<uint64_t>(output.data.empty() ? 0 : output.data.back());
      }
      throw std::invalid_argument("unknown benchmark mode");
    };
    for (size_t i = 0; i < 100; ++i) { auto value = operation(); escape(value); }
    std::vector<double> batches;
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
    if (mode == "copy_reuse" && destination != source) throw std::runtime_error("full copy oracle failed");
    std::sort(batches.begin(), batches.end());
    auto percentile = [&](size_t percent) { return batches[((batches.size() - 1) * percent + 99) / 100]; };
    std::cout << "{\"implementation\":\"rclcpp\",\"rmw\":\"" << rmw_get_implementation_identifier()
      << "\",\"mode\":\"" << mode << "\",\"elements\":" << elements
      << ",\"iterations\":" << iterations << ",\"elapsed_ns\":" << ns << ",\"ns_per_op\":" << static_cast<double>(ns) / iterations
      << ",\"batch_ns_per_op_p50\":" << percentile(50) << ",\"batch_ns_per_op_p95\":" << percentile(95)
      << ",\"batch_ns_per_op_p99\":" << percentile(99) << ",\"wire_bytes\":" << wire.buffer_length
      << ",\"wire_hash\":" << fingerprint(wire.buffer, wire.buffer_length) << ",\"checksum\":" << checksum
      << ",\"native_copy_pointer_changes\":" << pointer_changes << "}\n";
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}

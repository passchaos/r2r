#include "codec_support/run.hpp"
#include <std_msgs/msg/int32_multi_array.hpp>
struct Integers {
  using Message = std_msgs::msg::Int32MultiArray;
  static constexpr const char *name = "int32_array";
  static Message message(size_t elements) {
    Message source;
    source.layout.dim.resize(1);
    source.layout.dim[0].label = "samples";
    source.layout.dim[0].size = static_cast<uint32_t>(elements);
    source.layout.dim[0].stride = static_cast<uint32_t>(elements);
    source.data.resize(elements);
    for (size_t i = 0; i < elements; ++i) source.data[i] = static_cast<int32_t>((i * 17 + 3) % 100003) - 50000;
    return source;
  }
  static auto pointers(const Message &value) {
    return std::make_pair(value.data.data(), value.layout.dim.data());
  }
  static uint64_t tag(const Message &value) { return value.data.size() ^ value.layout.dim.size(); }
  static uint64_t owned_tag(const Message &value) {
    return value.data.size() ^ static_cast<uint64_t>(value.data.empty() ? 0 : value.data.back());
  }
};
int main(int argc, char **argv) { return run_codec<Integers>(argc, argv); }

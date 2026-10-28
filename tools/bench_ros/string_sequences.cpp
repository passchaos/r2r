#include "codec_support/run.hpp"
#include <rcl_interfaces/msg/parameter_value.hpp>
#include <iomanip>
#include <sstream>
struct Strings {
  using Message = rcl_interfaces::msg::ParameterValue;
  static constexpr const char *name = "string_array";
  static Message message(size_t elements) {
    Message source;
    source.type = 9;
    for (size_t i = 0; i < elements; ++i) {
      std::ostringstream text;
      text << "joint_" << std::setfill('0') << std::setw(4) << i << "_日本語_🦀_name";
      source.string_array_value.push_back(text.str());
    }
    return source;
  }
  static auto pointers(const Message &value) {
    return std::make_pair(value.string_array_value.data(), nullptr);
  }
  static uint64_t tag(const Message &value) { return value.string_array_value.size(); }
  static uint64_t owned_tag(const Message &value) {
    return value.string_array_value.size() ^ (value.string_array_value.empty() ? 0 : value.string_array_value.back().size());
  }
};
int main(int argc, char **argv) { return run_codec<Strings>(argc, argv); }

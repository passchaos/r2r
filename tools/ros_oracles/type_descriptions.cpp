#include <chrono>
#include <iostream>
#include <string>
#include <algorithm>
#include <fstream>
#include <iterator>
#include <rclcpp/rclcpp.hpp>
#include <std_msgs/msg/u_int64.hpp>
#include <type_description_interfaces/srv/get_type_description.hpp>
#include <rosidl_runtime_cpp/traits.hpp>
#include <rosidl_runtime_c/type_hash.h>
#include <rcl/type_hash.h>
#include <type_description_interfaces/msg/detail/type_description__functions.h>
#include <type_description_interfaces/msg/detail/field__functions.h>
#include <rosidl_runtime_c/string_functions.h>

using Service = type_description_interfaces::srv::GetTypeDescription;
static std::string hash_string() {
  auto support = rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::UInt64>();
  auto hash = support->get_type_hash_func(support);
  static const char hex[] = "0123456789abcdef";
  std::string result = "RIHS01_";
  if (hash->version != 1) { throw std::runtime_error("unsupported hash version"); }
  for (auto value : hash->value) { result += hex[value >> 4]; result += hex[value & 15]; }
  return result;
}

static void verify_schema_hash(const type_description_interfaces::msg::TypeDescription &schema) {
  using Native = type_description_interfaces__msg__TypeDescription;
  std::unique_ptr<Native, decltype(&type_description_interfaces__msg__TypeDescription__destroy)> native(
    type_description_interfaces__msg__TypeDescription__create(), type_description_interfaces__msg__TypeDescription__destroy);
  if (!native || !schema.referenced_type_descriptions.empty()) { throw std::runtime_error("invalid UInt64 reference graph"); }
  if (!rosidl_runtime_c__String__assign(&native->type_description.type_name, schema.type_description.type_name.c_str()) ||
    !type_description_interfaces__msg__Field__Sequence__init(&native->type_description.fields, schema.type_description.fields.size())) { throw std::bad_alloc(); }
  for (size_t i = 0; i < schema.type_description.fields.size(); ++i) {
    const auto &field = schema.type_description.fields[i]; auto &output = native->type_description.fields.data[i];
    if (!rosidl_runtime_c__String__assign(&output.name, field.name.c_str()) ||
      !rosidl_runtime_c__String__assign(&output.default_value, field.default_value.c_str()) ||
      !rosidl_runtime_c__String__assign(&output.type.nested_type_name, field.type.nested_type_name.c_str())) { throw std::bad_alloc(); }
    output.type.type_id = field.type.type_id; output.type.capacity = field.type.capacity; output.type.string_capacity = field.type.string_capacity;
  }
  rosidl_type_hash_t calculated{};
  if (rcl_calculate_type_hash(native.get(), &calculated) != RCL_RET_OK) { throw std::runtime_error("schema hash calculation failed"); }
  auto support = rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::UInt64>();
  const auto *expected = support->get_type_hash_func(support);
  if (calculated.version != expected->version || !std::equal(std::begin(calculated.value), std::end(calculated.value), std::begin(expected->value))) { throw std::runtime_error("schema hash mismatch"); }
}

int main(int argc, char **argv) {
  rclcpp::init(argc, argv);
  if (argc != 3) { return 2; }
  const std::string role = argv[1], name = argv[2];
  const auto hash = hash_string();
  if (role == "server") {
    auto node = std::make_shared<rclcpp::Node>(name);
    auto publisher = node->create_publisher<std_msgs::msg::UInt64>("/" + name + "/values", 10);
    std::cout << "{\"kind\":\"ready\",\"role\":\"server\",\"hash\":\"" << hash << "\"}" << std::endl;
    rclcpp::spin(node);
    return 0;
  }
  auto node = std::make_shared<rclcpp::Node>("type_description_cpp_client");
  auto client = node->create_client<Service>("/" + name + "/get_type_description");
  bool exact_source = false;
  std::string source_hex, source_encoding;
  if (!client->wait_for_service(std::chrono::seconds(10))) { throw std::runtime_error("description service missing"); }
  for (bool sources : {true, false}) {
    auto request = std::make_shared<Service::Request>();
    request->type_name = "std_msgs/msg/UInt64"; request->type_hash = hash; request->include_type_sources = sources;
    auto future = client->async_send_request(request);
    if (rclcpp::spin_until_future_complete(node, future, std::chrono::seconds(10)) != rclcpp::FutureReturnCode::SUCCESS) { throw std::runtime_error("description request timeout"); }
    auto response = future.get();
    verify_schema_hash(response->type_description);
    if (!response->successful || response->type_description.type_description.type_name != request->type_name || response->type_description.type_description.fields.size() != 1 || response->type_description.type_description.fields[0].name != "data") { throw std::runtime_error("schema identity mismatch"); }
    if (sources && !std::any_of(response->type_sources.begin(), response->type_sources.end(), [](const auto &s) { return s.type_name == "std_msgs/msg/UInt64" && !s.raw_file_contents.empty(); })) { throw std::runtime_error("source missing"); }
    if (sources) {
      const auto source = std::find_if(response->type_sources.begin(), response->type_sources.end(), [](const auto &s) { return s.type_name == "std_msgs/msg/UInt64"; });
      const std::string path = std::string("/opt/ros/") + std::getenv("ROS_DISTRO") + "/share/std_msgs/msg/UInt64." + source->encoding;
      std::ifstream file(path);
      if (!file) { throw std::runtime_error("installed interface source missing"); }
      const std::string contents((std::istreambuf_iterator<char>(file)), std::istreambuf_iterator<char>());
      exact_source = contents == source->raw_file_contents;
      source_encoding = source->encoding;
      for (unsigned char byte : source->raw_file_contents) { source_hex += "0123456789abcdef"[byte >> 4]; source_hex += "0123456789abcdef"[byte & 15]; }
    }
    if (!sources && !response->type_sources.empty()) { throw std::runtime_error("source omission failed"); }
  }
  std::cout << "{\"kind\":\"result\",\"implementation\":\"rclcpp\",\"type_name\":\"std_msgs/msg/UInt64\",\"hash\":\"" << hash << "\",\"fields\":1,\"sources\":true,\"omission\":true,\"exact_source\":" << (exact_source ? "true" : "false") << ",\"encoding\":\"" << source_encoding << "\",\"source_hex\":\"" << source_hex << "\"}" << std::endl;
  rclcpp::shutdown();
}

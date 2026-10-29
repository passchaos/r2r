#include <chrono>
#include <iostream>
#include <string>
#include <rclcpp/rclcpp.hpp>
#include <std_msgs/msg/u_int64.hpp>

int main(int argc, char **argv) {
  rclcpp::init(argc, argv);
  if (argc != 2) { std::cerr << "usage: qos_events <topic-prefix>\n"; return 2; }
  const std::string prefix = argv[1];
  auto node = std::make_shared<rclcpp::Node>("native_qos_event_peer");
  int offered = 0, requested = 0;
  bool policies_valid = true;
  rclcpp::PublisherOptions pub_options;
  pub_options.event_callbacks.incompatible_qos_callback = [&](rclcpp::QOSOfferedIncompatibleQoSInfo &info) {
    offered += info.total_count_change;
    policies_valid &= info.last_policy_kind == RMW_QOS_POLICY_RELIABILITY;
  };
  rclcpp::SubscriptionOptions sub_options;
  sub_options.event_callbacks.incompatible_qos_callback = [&](rclcpp::QOSRequestedIncompatibleQoSInfo &info) {
    requested += info.total_count_change;
    policies_valid &= info.last_policy_kind == RMW_QOS_POLICY_RELIABILITY;
  };
  auto publisher = node->create_publisher<std_msgs::msg::UInt64>(
    prefix + "/rust_requested", rclcpp::QoS(10).best_effort(), pub_options);
  auto subscription = node->create_subscription<std_msgs::msg::UInt64>(
    prefix + "/rust_offered", rclcpp::QoS(10).reliable(),
    [](std_msgs::msg::UInt64::ConstSharedPtr) { throw std::runtime_error("incompatible data delivery"); }, sub_options);
  rclcpp::executors::SingleThreadedExecutor executor;
  executor.add_node(node);
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
  while ((offered == 0 || requested == 0) && std::chrono::steady_clock::now() < deadline) {
    executor.spin_once(std::chrono::milliseconds(100));
  }
  if (offered != 1 || requested != 1 || !policies_valid) {
    std::cerr << "native QoS oracle failed " << offered << "/" << requested << "\n"; return 1;
  }
  std::cout << "{\"implementation\":\"rclcpp\",\"offered_count\":1,\"requested_count\":1,\"policy\":\"reliability\",\"execution\":\"native_wait\"}" << std::endl;
  std::string finish;
  if (!std::getline(std::cin, finish) || finish != "finish") { return 2; }
  rclcpp::shutdown();
  return 0;
}

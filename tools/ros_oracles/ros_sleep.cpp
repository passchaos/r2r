#include <chrono>
#include <future>
#include <iostream>
#include <sstream>
#include <string>
#include <thread>
#include <rclcpp/rclcpp.hpp>
#include <rosgraph_msgs/msg/clock.hpp>

int main(int argc, char **argv) {
  rclcpp::init(argc, argv);
  rclcpp::NodeOptions options;
  options.parameter_overrides({rclcpp::Parameter("use_sim_time", true)});
  auto node = std::make_shared<rclcpp::Node>("cpp_ros_sleep_oracle", options);
  auto clock = node->get_clock();
  auto publisher = node->create_publisher<rosgraph_msgs::msg::Clock>("/clock", 10);
  rclcpp::executors::SingleThreadedExecutor executor;
  executor.add_node(node);
  std::thread runner([&] { executor.spin(); });
  const auto ready_deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (publisher->get_subscription_count() == 0) {
    if (std::chrono::steady_clock::now() > ready_deadline) { throw std::runtime_error("clock subscription missing"); }
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  std::future<bool> sleep_result;
  std::thread sleeper;
  int64_t target = 0;
  std::string state = "idle";
  std::cout << "{\"kind\":\"ready\",\"implementation\":\"rclcpp\"}" << std::endl;
  for (std::string line; std::getline(std::cin, line);) {
    std::istringstream input(line); std::string command; input >> command;
    if (command == "clock") {
      int64_t now; input >> now;
      rosgraph_msgs::msg::Clock message;
      message.clock.sec = static_cast<int32_t>(now / 1000000000);
      message.clock.nanosec = static_cast<uint32_t>(now % 1000000000);
      publisher->publish(message);
      const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
      while (clock->now().nanoseconds() != now) {
        if (std::chrono::steady_clock::now() > deadline) { throw std::runtime_error("native clock sample did not arrive"); }
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    } else if (command == "sleep") {
      input >> target;
      if (sleeper.joinable()) { throw std::runtime_error("previous sleep is pending"); }
      std::promise<bool> result;
      sleep_result = result.get_future();
      sleeper = std::thread([clock, target, result = std::move(result)]() mutable {
        result.set_value(clock->sleep_until(rclcpp::Time(target, RCL_ROS_TIME)));
      });
      state = "pending";
    } else if (command == "pause") {
      std::this_thread::sleep_for(std::chrono::milliseconds(50));
    } else if (command == "cancel") {
      clock->cancel_sleep_or_wait();
    } else if (command != "finish") { throw std::runtime_error("invalid trace command"); }
    if (sleep_result.valid()) {
      const bool due = clock->now().nanoseconds() >= target || command == "cancel";
      if (due && sleep_result.wait_for(std::chrono::seconds(5)) != std::future_status::ready) { throw std::runtime_error("clock sleep did not finish"); }
      if (sleep_result.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
        state = sleep_result.get() ? "complete" : "canceled";
        sleeper.join();
      }
    }
    std::cout << "{\"command\":\"" << line << "\",\"now_ns\":" << clock->now().nanoseconds() << ",\"state\":\"" << state << "\"}" << std::endl;
    if (command == "finish") { break; }
  }
  if (sleeper.joinable()) { clock->cancel_sleep_or_wait(); sleeper.join(); }
  executor.cancel(); runner.join(); rclcpp::shutdown();
  return 0;
}

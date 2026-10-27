#include <algorithm>
#include <chrono>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include <rclcpp/rclcpp.hpp>
#include <rmw/rmw.h>
#include <std_msgs/msg/u_int64_multi_array.hpp>

using Message = std_msgs::msg::UInt64MultiArray;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;

uint64_t payload(size_t index) { return (index * 17 + 3) % 100003; }

void validate(const Message &message, size_t elements, size_t sequence) {
  if (!message.layout.dim.empty() || message.layout.data_offset != 0 ||
      message.data.size() != elements + 1 || message.data[0] != sequence)
    throw std::runtime_error("sequence or message shape oracle failed");
  for (size_t i = 0; i < elements; ++i)
    if (message.data[i + 1] != payload(i)) throw std::runtime_error("payload oracle failed");
}

int main(int argc, char **argv) {
  try {
    if (argc != 6) throw std::invalid_argument("usage: rclcpp_roundtrip <ping|echo> <payload-elements> <iterations> <warmup> <topic-prefix>");
    const std::string role = argv[1], prefix = argv[5];
    const size_t elements = std::stoull(argv[2]), iterations = std::stoull(argv[3]), warmup = std::stoull(argv[4]);
    if ((role != "ping" && role != "echo") || !iterations || elements > 1000000 || warmup > UINT32_MAX || iterations > UINT32_MAX - warmup)
      throw std::invalid_argument("invalid roundtrip configuration");
    const size_t total = iterations + warmup;
    rclcpp::init(argc, argv);
    rclcpp::NodeOptions options;
    options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node = std::make_shared<rclcpp::Node>("rclcpp_roundtrip_" + role, options);
    auto qos = rclcpp::QoS(rclcpp::KeepLast(16)).reliable().durability_volatile();
    auto publisher = node->create_publisher<Message>(prefix + (role == "ping" ? "/request" : "/response"), qos);
    Message::ConstSharedPtr incoming;
    auto subscription = node->create_subscription<Message>(prefix + (role == "ping" ? "/response" : "/request"), qos,
      [&](Message::ConstSharedPtr message) {
        if (incoming) throw std::runtime_error("unexpected duplicate response");
        incoming = std::move(message);
      });
    rclcpp::executors::SingleThreadedExecutor executor;
    executor.add_node(node);
    auto deadline = Clock::now() + 10s;
    while (publisher->get_subscription_count() != 1 || subscription->get_publisher_count() != 1) {
      if (Clock::now() >= deadline) throw std::runtime_error("endpoint discovery timed out or unexpected peers");
      executor.spin_once(10ms);
    }
    auto receive = [&]() {
      auto deadline = Clock::now() + 10s;
      while (!incoming) {
        if (Clock::now() >= deadline) throw std::runtime_error("message receive timed out");
        executor.spin_once(100ms);
      }
      auto message = std::move(incoming);
      incoming.reset();
      return message;
    };
    uint64_t checksum = 0;
    std::vector<uint64_t> samples;
    samples.reserve(iterations);
    if (role == "echo") {
      for (size_t sequence = 0; sequence < total; ++sequence) {
        auto message = receive();
        validate(*message, elements, sequence);
        checksum += message->data[0];
        publisher->publish(*message);
      }
      auto done = receive();
      if (!done->data.empty() || !done->layout.dim.empty() || done->layout.data_offset)
        throw std::runtime_error("completion oracle failed");
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"echo\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"elements\":" << elements << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << total << ",\"checksum\":" << checksum << "}\n";
    } else {
      Message message;
      message.data.resize(elements + 1);
      for (size_t i = 0; i < elements; ++i) message.data[i + 1] = payload(i);
      auto measured_start = Clock::now();
      for (size_t sequence = 0; sequence < total; ++sequence) {
        message.data[0] = sequence;
        if (sequence == warmup) measured_start = Clock::now();
        auto before = Clock::now();
        publisher->publish(message);
        auto response = receive();
        uint64_t elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - before).count();
        validate(*response, elements, sequence);
        checksum += response->data[0];
        if (sequence >= warmup) samples.push_back(elapsed);
      }
      const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - measured_start).count();
      publisher->publish(Message{});
      if (!publisher->wait_for_all_acked(10s)) throw std::runtime_error("completion acknowledgement timed out");
      auto sorted = samples;
      std::sort(sorted.begin(), sorted.end());
      auto percentile = [&](size_t percent) { return sorted[((sorted.size() - 1) * percent + 99) / 100]; };
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"ping\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"elements\":" << elements << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << total << ",\"checksum\":" << checksum << ",\"elapsed_ns\":" << elapsed
        << ",\"roundtrips_per_second\":" << static_cast<double>(iterations) / (elapsed * 1e-9)
        << ",\"rtt_ns_p50\":" << percentile(50) << ",\"rtt_ns_p95\":" << percentile(95)
        << ",\"rtt_ns_p99\":" << percentile(99) << ",\"rtt_ns_max\":" << sorted.back() << ",\"samples_ns\":[";
      for (size_t i = 0; i < samples.size(); ++i) { if (i) std::cout << ','; std::cout << samples[i]; }
      std::cout << "]}\n";
    }
    rclcpp::shutdown();
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}

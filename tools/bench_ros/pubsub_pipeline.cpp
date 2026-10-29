#include <algorithm>
#include <chrono>
#include <cstdint>
#include <deque>
#include <iomanip>
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

uint64_t elapsed_ns(Clock::time_point before) {
  return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - before).count();
}

uint64_t percentile(std::vector<uint64_t> values, size_t percent) {
  std::sort(values.begin(), values.end());
  return values[((values.size() - 1) * percent + 99) / 100];
}

void render_array(const std::vector<uint64_t> &values) {
  std::cout << '[';
  for (size_t i = 0; i < values.size(); ++i) {
    if (i) std::cout << ',';
    std::cout << values[i];
  }
  std::cout << ']';
}

struct PingPhase {
  uint64_t checksum = 0;
  std::vector<uint64_t> samples;
  std::vector<uint64_t> publish_samples;
  uint64_t elapsed = 0;
  size_t max_inflight = 0;
};

int main(int argc, char **argv) {
  try {
    if (argc != 8)
      throw std::invalid_argument("usage: rclcpp_pubsub_pipeline <ping|echo> <payload-elements> <window> <fanout> <iterations> <warmup> <topic-prefix>");
    const std::string role = argv[1], prefix = argv[7];
    const size_t elements = std::stoull(argv[2]), window = std::stoull(argv[3]);
    const size_t fanout = std::stoull(argv[4]), iterations = std::stoull(argv[5]);
    const size_t warmup = std::stoull(argv[6]);
    if ((role != "ping" && role != "echo") || !iterations || !window || !fanout ||
        elements > 1000000 || window > 4096 || fanout > 64 || window > 4096 / fanout ||
        warmup > UINT32_MAX || iterations > UINT32_MAX - warmup)
      throw std::invalid_argument("invalid pipeline configuration");
    const size_t total = iterations + warmup, depth = std::max<size_t>(16, window);
    const uint64_t checksum = static_cast<uint64_t>(total) * (total - 1) / 2;
    rclcpp::init(argc, argv);
    rclcpp::NodeOptions options;
    options.enable_rosout(false).start_parameter_services(false)
      .start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node = std::make_shared<rclcpp::Node>("rclcpp_pipeline_" + role, options);
    auto qos = rclcpp::QoS(rclcpp::KeepLast(depth)).reliable().durability_volatile();
    const std::string publish_topic = prefix + (role == "ping" ? "/request" : "/response");
    const std::string subscribe_topic = prefix + (role == "ping" ? "/response" : "/request");
    auto publisher = node->create_publisher<Message>(publish_topic, qos);
    const size_t subscription_count = role == "ping" ? 1 : fanout;
    std::vector<std::deque<Message::ConstSharedPtr>> queues(subscription_count);
    std::vector<rclcpp::Subscription<Message>::SharedPtr> subscriptions;
    subscriptions.reserve(subscription_count);
    for (size_t index = 0; index < subscription_count; ++index) {
      subscriptions.push_back(node->create_subscription<Message>(subscribe_topic, qos,
        [&, index](Message::ConstSharedPtr message) { queues[index].push_back(std::move(message)); }));
    }
    rclcpp::executors::SingleThreadedExecutor executor;
    executor.add_node(node);
    auto deadline = Clock::now() + 30s;
    const size_t expected_publish_subscribers = role == "ping" ? fanout : 1;
    while (publisher->get_subscription_count() != expected_publish_subscribers ||
           std::any_of(subscriptions.begin(), subscriptions.end(),
             [](const auto &subscription) { return subscription->get_publisher_count() != 1; })) {
      if (Clock::now() >= deadline)
        throw std::runtime_error("endpoint discovery timed out or unexpected peers");
      executor.spin_once(10ms);
    }
    auto receive = [&](size_t index) {
      auto receive_deadline = Clock::now() + 30s;
      while (queues[index].empty()) {
        if (Clock::now() >= receive_deadline) throw std::runtime_error("message receive timed out");
        executor.spin_once(100ms);
      }
      auto message = std::move(queues[index].front());
      queues[index].pop_front();
      return message;
    };

    if (role == "echo") {
      Message::ConstSharedPtr response;
      std::vector<uint64_t> collect_samples, publish_samples;
      collect_samples.reserve(iterations);publish_samples.reserve(iterations);
      for (size_t sequence = 0; sequence < total; ++sequence) {
        const auto before = Clock::now();
        for (size_t index = 0; index < fanout; ++index) {
          auto message = receive(index);
          validate(*message, elements, sequence);
          if (!index) response = std::move(message);
        }
        if (sequence >= warmup) collect_samples.push_back(elapsed_ns(before));
        const auto publish_before = Clock::now();
        publisher->publish(*response);
        if (sequence >= warmup) publish_samples.push_back(elapsed_ns(publish_before));
      }
      for (size_t index = 0; index < fanout; ++index) {
        auto done = receive(index);
        if (!done->data.empty() || !done->layout.dim.empty() || done->layout.data_offset)
          throw std::runtime_error("completion oracle failed");
      }
      std::cout << std::setprecision(17) << "{\"implementation\":\"rclcpp\",\"role\":\"echo\",\"rmw\":\""
        << rmw_get_implementation_identifier() << "\",\"receive_path\":\"callback\",\"elements\":" << elements
        << ",\"window\":" << window << ",\"fanout\":" << fanout
        << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << total << ",\"fanout_received\":" << total * fanout
        << ",\"checksum\":" << checksum << ",\"fanout_checksum\":" << checksum * fanout
        << ",\"max_inflight\":" << std::min(window, std::max(iterations, warmup))
        << ",\"collect_ns_p50\":" << percentile(collect_samples, 50)
        << ",\"collect_ns_p95\":" << percentile(collect_samples, 95)
        << ",\"collect_ns_p99\":" << percentile(collect_samples, 99)
        << ",\"publish_ns_p50\":" << percentile(publish_samples, 50)
        << ",\"publish_ns_p95\":" << percentile(publish_samples, 95)
        << ",\"publish_ns_p99\":" << percentile(publish_samples, 99)
        << ",\"collect_samples_ns\":";render_array(collect_samples);
      std::cout << ",\"publish_samples_ns\":";render_array(publish_samples);std::cout << "}\n";
      rclcpp::shutdown();
      return 0;
    }

    Message message;
    message.data.resize(elements + 1);
    for (size_t i = 0; i < elements; ++i) message.data[i + 1] = payload(i);
    auto run_phase = [&](size_t first_sequence, size_t count, bool measured) {
      PingPhase phase;
      if (!count) return phase;
      std::vector<Clock::time_point> sent(count);
      phase.samples.reserve(measured ? count : 0);phase.publish_samples.reserve(measured ? count : 0);
      size_t next = 0, received = 0, inflight = 0;
      const auto started = Clock::now();
      auto publish = [&](size_t local_sequence) {
        message.data[0] = first_sequence + local_sequence;
        const auto before = Clock::now();
        publisher->publish(message);
        sent[local_sequence] = before;
        if (measured) phase.publish_samples.push_back(elapsed_ns(before));
        ++next;++inflight;phase.max_inflight = std::max(phase.max_inflight, inflight);
      };
      while (next < count && inflight < window) publish(next);
      while (received < count) {
        auto response = receive(0);
        validate(*response, elements, first_sequence + received);
        if (measured) phase.samples.push_back(elapsed_ns(sent[received]));
        phase.checksum += response->data[0];
        ++received;--inflight;
        if (next < count) publish(next);
      }
      phase.elapsed = elapsed_ns(started);
      return phase;
    };
    auto warmup_phase = run_phase(0, warmup, false);
    auto measured = run_phase(warmup, iterations, true);
    publisher->publish(Message{});
    if (!publisher->wait_for_all_acked(30s))
      throw std::runtime_error("completion acknowledgement timed out");
    std::cout << std::setprecision(17) << "{\"implementation\":\"rclcpp\",\"role\":\"ping\",\"rmw\":\""
      << rmw_get_implementation_identifier() << "\",\"receive_path\":\"callback\",\"elements\":" << elements
      << ",\"window\":" << window << ",\"fanout\":" << fanout
      << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
      << ",\"received\":" << total << ",\"fanout_received\":" << total * fanout
      << ",\"checksum\":" << warmup_phase.checksum + measured.checksum
      << ",\"fanout_checksum\":" << checksum * fanout
      << ",\"max_inflight\":" << std::max(warmup_phase.max_inflight, measured.max_inflight)
      << ",\"elapsed_ns\":" << measured.elapsed
      << ",\"messages_per_second\":" << iterations / (measured.elapsed * 1e-9)
      << ",\"rtt_ns_p50\":" << percentile(measured.samples, 50)
      << ",\"rtt_ns_p95\":" << percentile(measured.samples, 95)
      << ",\"rtt_ns_p99\":" << percentile(measured.samples, 99)
      << ",\"rtt_ns_max\":" << *std::max_element(measured.samples.begin(), measured.samples.end())
      << ",\"publish_ns_p50\":" << percentile(measured.publish_samples, 50)
      << ",\"publish_ns_p95\":" << percentile(measured.publish_samples, 95)
      << ",\"publish_ns_p99\":" << percentile(measured.publish_samples, 99)
      << ",\"samples_ns\":";render_array(measured.samples);
    std::cout << ",\"publish_samples_ns\":";render_array(measured.publish_samples);std::cout << "}\n";
    rclcpp::shutdown();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}

#include <algorithm>
#include <chrono>
#include <deque>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include <rclcpp/rclcpp.hpp>
#include <rclcpp/version.h>
#include <rmw/rmw.h>
#include <example_interfaces/srv/add_two_ints.hpp>

using Service = example_interfaces::srv::AddTwoInts;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;

int64_t operand(size_t sequence) { return static_cast<int64_t>((sequence * 17 + 3) % 100003) - 50000; }
int64_t expected_sum(size_t sequence) { return static_cast<int64_t>(sequence) + operand(sequence); }

int main(int argc, char **argv) {
  try {
    if (argc != 6) throw std::invalid_argument("usage: rclcpp_service <ping|echo> <window> <iterations> <warmup> <service-name>");
    const std::string role = argv[1], name = argv[5];
    const size_t window = std::stoull(argv[2]), iterations = std::stoull(argv[3]), warmup = std::stoull(argv[4]);
    if ((role != "ping" && role != "echo") || window < 1 || window > 1024 || !iterations || warmup > UINT32_MAX || iterations > UINT32_MAX - warmup)
      throw std::invalid_argument("invalid service benchmark configuration");
    const size_t total = iterations + warmup;
    rclcpp::init(argc, argv);
    rclcpp::NodeOptions options;
    options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node = std::make_shared<rclcpp::Node>("rclcpp_service_" + role, options);
    rclcpp::executors::SingleThreadedExecutor executor;
    executor.add_node(node);
#if RCLCPP_VERSION_MAJOR < 28
    const auto qos = rclcpp::ServicesQoS().keep_last(std::max<size_t>(16, window)).get_rmw_qos_profile();
#else
    const auto qos = rclcpp::ServicesQoS().keep_last(std::max<size_t>(16, window));
#endif
    uint64_t checksum = 0;
    if (role == "echo") {
      size_t received = 0;
      bool stopping = false, confirmed = false;
      auto service = node->create_service<Service>(name,
        [&](const std::shared_ptr<Service::Request> request, std::shared_ptr<Service::Response> response) {
          if (received < total) {
            if (request->a != static_cast<int64_t>(received) || request->b != operand(received)) throw std::runtime_error("request sequence/operand oracle failed");
            response->sum = expected_sum(received);
            checksum += received++;
          } else if (!stopping) {
            if (request->a != -1 || request->b != 0) throw std::runtime_error("completion request oracle failed");
            response->sum = -1;
            stopping = true;
          } else {
            if (confirmed || request->a != -2 || request->b != 0) throw std::runtime_error("completion confirmation oracle failed");
            response->sum = -2;
            confirmed = true;
          }
        }, qos);
      auto deadline = Clock::now() + 10s;
      size_t previous_count = 0;
      bool previous_stopping = false;
      while (!confirmed) {
        executor.spin_once(100ms);
        if (received != previous_count || stopping != previous_stopping) {
          deadline = Clock::now() + 10s;
          previous_count = received;
          previous_stopping = stopping;
        }
        if (Clock::now() >= deadline) throw std::runtime_error("service request deadline expired");
      }
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"echo\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"window\":" << window << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << received << ",\"checksum\":" << checksum << "}\n";
    } else {
      auto client = node->create_client<Service>(name, qos);
      auto deadline = Clock::now() + 10s;
      while (!client->service_is_ready()) {
        if (Clock::now() >= deadline) throw std::runtime_error("service discovery timed out");
        executor.spin_once(10ms);
      }
      std::vector<uint64_t> samples;
      samples.reserve(iterations);
      using Future = rclcpp::Client<Service>::FutureAndRequestId;
      struct Pending { size_t sequence; Clock::time_point before; Future response; };
      auto drive = [&](size_t start, size_t count, bool measured) {
        std::deque<Pending> pending;
        size_t sent = 0, completed = 0;
        while (completed < count) {
          while (sent < count && pending.size() < window) {
            size_t sequence = start + sent;
            auto request = std::make_shared<Service::Request>();
            request->a = static_cast<int64_t>(sequence);
            request->b = operand(sequence);
            auto before = Clock::now();
            auto response = client->async_send_request(request);
            pending.push_back({sequence, before, std::move(response)});
            ++sent;
          }
          auto &front = pending.front();
          if (front.response.wait_for(0s) == std::future_status::ready) {
            auto response = front.response.get();
            auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - front.before).count();
            if (response->sum != expected_sum(front.sequence)) throw std::runtime_error("response sum oracle failed");
            checksum += front.sequence;
            if (measured) samples.push_back(elapsed);
            pending.pop_front();
            ++completed;
          } else {
            if (Clock::now() - front.before >= 10s) throw std::runtime_error("service response deadline expired");
            executor.spin_once(100ms);
          }
        }
      };
      drive(0, warmup, false);
      auto start = Clock::now();
      drive(warmup, iterations, true);
      const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - start).count();
      auto stop = std::make_shared<Service::Request>();
      stop->a = -1;
      auto stopped = client->async_send_request(stop);
      deadline = Clock::now() + 10s;
      while (stopped.wait_for(0s) != std::future_status::ready) {
        if (Clock::now() >= deadline) throw std::runtime_error("completion response deadline expired");
        executor.spin_once(100ms);
      }
      if (stopped.get()->sum != -1) throw std::runtime_error("completion response oracle failed");
      auto confirm = std::make_shared<Service::Request>();
      confirm->a = -2;
      auto confirmation = client->async_send_request(confirm);
      client->remove_pending_request(confirmation);
      auto sorted = samples;
      std::sort(sorted.begin(), sorted.end());
      auto percentile = [&](size_t percent) { return sorted[((sorted.size() - 1) * percent + 99) / 100]; };
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"ping\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"window\":" << window << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
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

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
#include <rclcpp_action/rclcpp_action.hpp>
#include <rmw/rmw.h>
#include <example_interfaces/action/fibonacci.hpp>
#include <example_interfaces/srv/add_two_ints.hpp>

using Action = example_interfaces::action::Fibonacci;
using Client = rclcpp_action::Client<Action>;
using ClientGoal = rclcpp_action::ClientGoalHandle<Action>;
using ServerGoal = rclcpp_action::ServerGoalHandle<Action>;
using Ready = example_interfaces::srv::AddTwoInts;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;

std::vector<int32_t> fixture(int32_t order) { return {order, order + 1, order + 2}; }
uint64_t elapsed_ns(Clock::time_point before) {
  return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - before).count();
}
uint64_t percentile(std::vector<uint64_t> values, size_t percent) {
  std::sort(values.begin(), values.end());
  return values[((values.size() - 1) * percent + 99) / 100];
}
void array_json(const std::vector<uint64_t> &values) {
  std::cout << '[';
  for (size_t i = 0; i < values.size(); ++i) { if (i) std::cout << ','; std::cout << values[i]; }
  std::cout << ']';
}

int main(int argc, char **argv) {
  try {
    if (argc != 6) throw std::invalid_argument("usage: rclcpp_action_roundtrip <ping|echo> <scenario:0=success,1=cancel> <iterations> <warmup> <action-name>");
    const std::string role = argv[1], name = argv[5];
    const size_t scenario = std::stoull(argv[2]), iterations = std::stoull(argv[3]), warmup = std::stoull(argv[4]);
    if ((role != "ping" && role != "echo") || scenario > 1 || !iterations || warmup > INT32_MAX - 3 || iterations > INT32_MAX - 3 - warmup)
      throw std::invalid_argument("invalid action configuration");
    const size_t total = iterations + warmup;
    rclcpp::init(argc, argv);
    rclcpp::NodeOptions options;
    options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node = std::make_shared<rclcpp::Node>("rclcpp_action_" + role, options);
    rclcpp::executors::SingleThreadedExecutor executor;
    executor.add_node(node);
    const auto qos = rclcpp::ServicesQoS().keep_last(16).get_rmw_qos_profile();
#if RCLCPP_VERSION_MAJOR < 28
    const auto ready_qos = qos;
#else
    const auto ready_qos = rclcpp::ServicesQoS().keep_last(16);
#endif
    uint64_t checksum = 0;
    std::vector<uint64_t> samples, acceptance, cancellation;
    samples.reserve(iterations); acceptance.reserve(iterations); cancellation.reserve(iterations);
    if (role == "echo") {
      std::shared_ptr<ServerGoal> active;
      size_t received = 0, controls = 0;
      bool start_requested = false, feedback_acknowledged = false;
      auto ready = node->create_service<Ready>(name + "/ready",
        [&](const std::shared_ptr<Ready::Request> request, std::shared_ptr<Ready::Response> response) {
          if (!active || request->a != active->get_goal()->order) throw std::runtime_error("execution handshake goal mismatch");
          if (request->b == 0 && !start_requested) { start_requested = true; }
          else if (request->b == 1 && !scenario && start_requested && !feedback_acknowledged) { feedback_acknowledged = true; }
          else { throw std::runtime_error("unexpected execution handshake"); }
          response->sum = request->a + request->b;
        }, ready_qos);
      auto server_options = rcl_action_server_get_default_options();
      server_options.goal_service_qos = qos;
      server_options.result_service_qos = qos;
      server_options.cancel_service_qos = qos;
      server_options.feedback_topic_qos = qos;
      server_options.result_timeout.nanoseconds = 10000000000LL;
      auto server = rclcpp_action::create_server<Action>(node, name,
        [&](const rclcpp_action::GoalUUID &, std::shared_ptr<const Action::Goal> goal) {
          if (received < total) {
            if (goal->order != static_cast<int32_t>(received + 2) || active)
              throw std::runtime_error("goal sequence or concurrency oracle failed");
            return rclcpp_action::GoalResponse::ACCEPT_AND_DEFER;
          }
          if (controls >= 2 || goal->order != -static_cast<int32_t>(controls + 1))
            throw std::runtime_error("completion goal oracle failed");
          ++controls;
          return rclcpp_action::GoalResponse::REJECT;
        },
        [&](std::shared_ptr<ServerGoal> goal) {
          if (!scenario || !active || goal != active) throw std::runtime_error("unexpected cancellation");
          return rclcpp_action::CancelResponse::ACCEPT;
        },
        [&](std::shared_ptr<ServerGoal> goal) {
          if (active) throw std::runtime_error("duplicate accepted goal");
          active = std::move(goal);
        }, server_options);
      for (size_t sequence = 0; sequence < total; ++sequence) {
        auto deadline = Clock::now() + 10s;
        while (!active) {
          if (Clock::now() >= deadline) throw std::runtime_error("goal deadline expired");
          executor.spin_once(100ms);
        }
        const int32_t order = static_cast<int32_t>(sequence + 2);
        if (active->get_goal()->order != order) throw std::runtime_error("accepted goal oracle failed");
        deadline = Clock::now() + 10s;
        while (!start_requested) {
          if (Clock::now() >= deadline) throw std::runtime_error("execution handshake deadline expired");
          executor.spin_once(100ms);
        }
        active->execute();
        auto feedback = std::make_shared<Action::Feedback>();
        feedback->sequence = fixture(order);
        active->publish_feedback(feedback);
        auto result = std::make_shared<Action::Result>();
        result->sequence = fixture(order);
        if (!scenario) {
          deadline = Clock::now() + 10s;
          while (!feedback_acknowledged) {
            if (Clock::now() >= deadline) throw std::runtime_error("feedback acknowledgement deadline expired");
            executor.spin_once(100ms);
          }
          active->succeed(result);
        }
        else {
          deadline = Clock::now() + 10s;
          while (!active->is_canceling()) {
            if (Clock::now() >= deadline) throw std::runtime_error("cancel deadline expired");
            executor.spin_once(100ms);
          }
          active->canceled(result);
        }
        active.reset();
        start_requested = false;
        feedback_acknowledged = false;
        checksum += sequence;
        ++received;
      }
      auto deadline = Clock::now() + 10s;
      while (controls < 2) {
        if (Clock::now() >= deadline) throw std::runtime_error("completion goal deadline expired");
        executor.spin_once(100ms);
      }
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"echo\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"scenario\":" << scenario << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << total << ",\"checksum\":" << checksum << ",\"accepted\":" << total
        << ",\"feedback\":" << total << ",\"results\":" << total << ",\"canceled\":" << (scenario ? total : 0) << "}\n";
    } else {
      auto client_options = rcl_action_client_get_default_options();
      client_options.goal_service_qos = qos;
      client_options.result_service_qos = qos;
      client_options.cancel_service_qos = qos;
      client_options.feedback_topic_qos = qos;
      auto client = rclcpp_action::create_client<Action>(node, name, nullptr, client_options);
      auto ready = node->create_client<Ready>(name + "/ready", ready_qos);
      auto deadline = Clock::now() + 10s;
      while (!client->action_server_is_ready() || !ready->service_is_ready()) {
        if (Clock::now() >= deadline) throw std::runtime_error("action discovery timed out");
        executor.spin_once(10ms);
      }
      auto drive = [&](auto &future) {
        auto deadline = Clock::now() + 10s;
        while (future.wait_for(0s) != std::future_status::ready) {
          if (Clock::now() >= deadline) throw std::runtime_error("action response deadline expired");
          executor.spin_once(100ms);
        }
        return future.get();
      };
      std::deque<std::vector<int32_t>> feedbacks;
      auto handshake = [&](int32_t order, int64_t phase) {
        auto request = std::make_shared<Ready::Request>();
        request->a = order;
        request->b = phase;
        auto response = ready->async_send_request(request);
        if (drive(response)->sum != order + phase) throw std::runtime_error("execution handshake response oracle failed");
      };
      auto measured_start = Clock::now();
      for (size_t sequence = 0; sequence < total; ++sequence) {
        if (sequence == warmup) measured_start = Clock::now();
        const int32_t order = static_cast<int32_t>(sequence + 2);
        Action::Goal goal;
        goal.order = order;
        Client::SendGoalOptions send_options;
        send_options.feedback_callback = [&](ClientGoal::SharedPtr, const std::shared_ptr<const Action::Feedback> feedback) {
          feedbacks.push_back(feedback->sequence);
        };
        auto before = Clock::now();
        auto goal_future = client->async_send_goal(goal, send_options);
        auto handle = drive(goal_future);
        if (!handle) throw std::runtime_error("goal unexpectedly rejected");
        auto result_future = client->async_get_result(handle);
        const auto accepted_ns = elapsed_ns(before);
        handshake(order, 0);
        deadline = Clock::now() + 10s;
        while (feedbacks.empty()) {
          if (Clock::now() >= deadline) throw std::runtime_error("feedback deadline expired");
          executor.spin_once(100ms);
        }
        if (feedbacks.front() != fixture(order)) throw std::runtime_error("feedback payload oracle failed");
        feedbacks.pop_front();
        if (scenario) {
          auto before_cancel = Clock::now();
          auto cancel_future = client->async_cancel_goal(handle);
          auto cancel_response = drive(cancel_future);
          if (cancel_response->return_code != 0 || cancel_response->goals_canceling.size() != 1 ||
              cancel_response->goals_canceling[0].goal_id.uuid != handle->get_goal_id())
            throw std::runtime_error("cancel response oracle failed");
          if (sequence >= warmup) cancellation.push_back(elapsed_ns(before_cancel));
        } else { handshake(order, 1); }
        auto result = drive(result_future);
        const auto elapsed = elapsed_ns(before);
        if (result.code != (scenario ? rclcpp_action::ResultCode::CANCELED : rclcpp_action::ResultCode::SUCCEEDED) ||
            result.goal_id != handle->get_goal_id() || result.result->sequence != fixture(order))
          throw std::runtime_error("terminal result oracle failed");
        if (!feedbacks.empty()) throw std::runtime_error("duplicate feedback");
        checksum += sequence;
        if (sequence >= warmup) { samples.push_back(elapsed); acceptance.push_back(accepted_ns); }
      }
      const auto elapsed = elapsed_ns(measured_start);
      Action::Goal stop;
      stop.order = -1;
      auto stopped = client->async_send_goal(stop);
      if (drive(stopped)) throw std::runtime_error("completion goal should be rejected");
      stop.order = -2;
      client->async_send_goal(stop);
      std::cout << "{\"implementation\":\"rclcpp\",\"role\":\"ping\",\"rmw\":\"" << rmw_get_implementation_identifier()
        << "\",\"scenario\":" << scenario << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup
        << ",\"received\":" << total << ",\"checksum\":" << checksum << ",\"accepted\":" << total
        << ",\"feedback\":" << total << ",\"results\":" << total << ",\"canceled\":" << (scenario ? total : 0)
        << ",\"elapsed_ns\":" << elapsed << ",\"roundtrips_per_second\":" << iterations / (elapsed * 1e-9)
        << ",\"rtt_ns_p50\":" << percentile(samples,50) << ",\"rtt_ns_p95\":" << percentile(samples,95)
        << ",\"rtt_ns_p99\":" << percentile(samples,99) << ",\"rtt_ns_max\":" << *std::max_element(samples.begin(),samples.end())
        << ",\"samples_ns\":";
      array_json(samples);
      std::cout << ",\"stage_samples_ns\":{\"acceptance\":";
      array_json(acceptance);
      std::cout << ",\"cancellation\":";
      array_json(cancellation);
      std::cout << "},\"acceptance_ns_p50\":" << percentile(acceptance,50) << ",\"cancellation_ns_p50\":";
      if (scenario) std::cout << percentile(cancellation,50); else std::cout << "null";
      std::cout << "}\n";
    }
    rclcpp::shutdown();
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}

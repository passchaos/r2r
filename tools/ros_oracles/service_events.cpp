#include <example_interfaces/srv/add_two_ints.hpp>
#include <rclcpp/rclcpp.hpp>
#include <rcl/time.h>
#include <rmw/rmw.h>

#include <chrono>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>
#include <unistd.h>

using Service = example_interfaces::srv::AddTwoInts;
using Event = example_interfaces::srv::AddTwoInts_Event;
using namespace std::chrono_literals;
constexpr size_t REQUESTS = 3;
constexpr int64_t CLOCK_NS = 12000000034;

static void check(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

template <class Bytes> static std::string guid(const Bytes &bytes) {
  std::ostringstream out;
  out << '[';
  for (size_t i = 0; i < 16; ++i) {
    if (i) out << ',';
    out << unsigned(static_cast<uint8_t>(bytes[i]));
  }
  out << ']';
  return out.str();
}

static std::string event_json(const Event &event) {
  std::ostringstream out;
  out << "{\"info\":{\"event_type\":" << unsigned(event.info.event_type)
      << ",\"stamp\":{\"sec\":" << event.info.stamp.sec
      << ",\"nanosec\":" << event.info.stamp.nanosec << "},\"sequence_number\":"
      << event.info.sequence_number << ",\"client_gid\":" << guid(event.info.client_gid)
      << "},\"request\":[";
  for (size_t i = 0; i < event.request.size(); ++i) {
    if (i) out << ',';
    out << "{\"a\":" << event.request[i].a << ",\"b\":" << event.request[i].b << '}';
  }
  out << "],\"response\":[";
  for (size_t i = 0; i < event.response.size(); ++i) {
    if (i) out << ',';
    out << "{\"sum\":" << event.response[i].sum << '}';
  }
  out << "]}";
  return out.str();
}

static std::string call_json(int64_t a, int64_t b, int64_t sequence,
                             const std::string &identity_key, const std::string &identity) {
  std::ostringstream out;
  out << "{\"a\":" << a << ",\"b\":" << b << ",\"sum\":" << a + b
      << ",\"sequence\":" << sequence << ",\"" << identity_key << "\":" << identity << '}';
  return out.str();
}

template <class Values> static void array(const Values &values) {
  std::cout << '[';
  for (size_t i = 0; i < values.size(); ++i) {
    if (i) std::cout << ',';
    std::cout << values[i];
  }
  std::cout << ']';
}

int main(int argc, char **argv) try {
  check(argc == 3, "usage: service_events <client|server> <service-name>; modes arrive on stdin");
  const std::string role = argv[1], name = argv[2], topic = name + "/_service_event";
  check(role == "client" || role == "server", "unknown endpoint role");
  check(!name.empty() && name.front() == '/', "service name must be absolute");
  // Initialize without passing the oracle's positional arguments as ROS arguments.
  rclcpp::init(0, nullptr);
  auto node = std::make_shared<rclcpp::Node>("service_events_cpp_" + role + "_" + std::to_string(getpid()));
  rclcpp::executors::SingleThreadedExecutor executor;
  executor.add_node(node);
  auto clock = std::make_shared<rclcpp::Clock>(RCL_ROS_TIME);
  check(rcl_enable_ros_time_override(clock->get_clock_handle()) == RCL_RET_OK, "enable ROS clock");
  check(rcl_set_ros_time_override(clock->get_clock_handle(), CLOCK_NS) == RCL_RET_OK, "set ROS clock");
  auto qos = rclcpp::QoS(16).reliable().transient_local();
  std::vector<std::string> events, calls;
  auto subscription = node->create_subscription<Event>(topic, qos, [&](const Event &event) {
    events.push_back(event_json(event));
  });
  rclcpp::Service<Service>::SharedPtr server;
  rclcpp::Client<Service>::SharedPtr client;
  if (role == "server") {
    server = node->create_service<Service>(name,
      [&](std::shared_ptr<rmw_request_id_t> header, std::shared_ptr<Service::Request> request,
          std::shared_ptr<Service::Response> response) {
        response->sum = request->a + request->b;
        calls.push_back(call_json(request->a, request->b, header->sequence_number,
                                  "writer_guid", guid(header->writer_guid)));
      });
  } else {
    client = node->create_client<Service>(name);
  }
  std::cout << "{\"kind\":\"started\",\"role\":\"" << role
            << "\",\"representation\":\"cpp\"}" << std::endl;
  std::string mode;
  size_t phase = 0;
  while (std::getline(std::cin, mode)) {
    auto state = RCL_SERVICE_INTROSPECTION_OFF;
    if (mode == "metadata") state = RCL_SERVICE_INTROSPECTION_METADATA;
    else if (mode == "contents") state = RCL_SERVICE_INTROSPECTION_CONTENTS;
    else check(mode == "off", "unknown introspection mode");
    events.clear();
    calls.clear();
    if (server) server->configure_introspection(clock, qos, state);
    else client->configure_introspection(clock, qos, state);
    std::cout << "{\"kind\":\"ready\",\"role\":\"" << role << "\",\"phase\":"
              << phase << ",\"mode\":\"" << mode << "\"}" << std::endl;
    const auto deadline = std::chrono::steady_clock::now() + 10s;
    auto spin = [&] {
      if (std::chrono::steady_clock::now() >= deadline) {
        throw std::runtime_error("phase " + std::to_string(phase) + " timed out: " +
                                 std::to_string(calls.size()) + " calls, " +
                                 std::to_string(events.size()) + " events");
      }
      executor.spin_once(10ms);
    };
    if (client) {
      while (!client->service_is_ready()) spin();
      rmw_gid_t identity{};
      auto handle = rcl_client_get_rmw_handle(client->get_client_handle().get());
      check(rmw_get_gid_for_client(handle, &identity) == RMW_RET_OK, "native client identity");
      for (size_t i = 0; i < REQUESTS; ++i) {
        auto request = std::make_shared<Service::Request>();
        request->a = static_cast<int64_t>(phase * REQUESTS + i + 1);
        request->b = 23;
        auto response = client->async_send_request(request);
        while (response.wait_for(0s) != std::future_status::ready) spin();
        check(response.get()->sum == request->a + request->b, "response sum mismatch");
        calls.push_back(call_json(request->a, request->b, response.request_id, "client_gid", guid(identity.data)));
      }
    }
    const size_t expected = mode == "off" ? 0 : REQUESTS * 4;
    while (calls.size() < REQUESTS || events.size() < expected) spin();
    const auto quiet = std::chrono::steady_clock::now() + 200ms;
    while (std::chrono::steady_clock::now() < quiet) spin();
    check(calls.size() == REQUESTS && events.size() == expected, "unexpected request/event count");
    const size_t publishers = expected == 0 ? 0 : 2;
    while (node->count_publishers(topic) != publishers) spin();
    std::cout << "{\"kind\":\"phase\",\"implementation\":\"rclcpp\",\"role\":\"" << role
              << "\",\"representation\":\"cpp\",\"service\":\"" << name
              << "\",\"phase\":" << phase << ",\"mode\":\"" << mode
              << "\",\"rmw\":\"" << rmw_get_implementation_identifier()
              << "\",\"clock_ns\":" << clock->now().nanoseconds()
              << ",\"event_publishers\":" << publishers << ",\"calls\":";
    array(calls);
    std::cout << ",\"events\":";
    array(events);
    std::cout << '}' << std::endl;
    ++phase;
  }
  executor.remove_node(node);
  rclcpp::shutdown();
  return 0;
} catch (const std::exception &error) {
  std::cerr << error.what() << '\n';
  return 1;
}

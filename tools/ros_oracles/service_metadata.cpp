#include <rcl/rcl.h>
#include <rcl/error_handling.h>
#include <rcl/graph.h>
#include <rmw/rmw.h>
#include <example_interfaces/srv/add_two_ints.h>

#include <chrono>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>
#include <unistd.h>

static void check(rcl_ret_t result, const char *operation) {
  if (result != RCL_RET_OK) {
    throw std::runtime_error(std::string(operation) + ": " + rcl_get_error_string().str);
  }
}

template <typename F> static void wait_for(F ready) {
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (!ready()) {
    if (std::chrono::steady_clock::now() >= deadline) {
      throw std::runtime_error("native service metadata oracle timed out");
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
}

template <typename Byte> static void guid(const Byte *data) {
  std::cout << '[';
  for (size_t i = 0; i < 16; ++i) {
    if (i) std::cout << ',';
    std::cout << static_cast<unsigned>(static_cast<uint8_t>(data[i]));
  }
  std::cout << ']';
}

int main() try {
  auto options = rcl_get_zero_initialized_init_options();
  const auto allocator = rcl_get_default_allocator();
  check(rcl_init_options_init(&options, allocator), "init options");
  auto context = rcl_get_zero_initialized_context();
  check(rcl_init(0, nullptr, &options, &context), "context");
  auto node = rcl_get_zero_initialized_node();
  auto node_options = rcl_node_get_default_options();
  const auto name = "service_metadata_oracle_" + std::to_string(getpid());
  check(rcl_node_init(&node, name.c_str(), "/", &context, &node_options), "node");
  auto service = rcl_get_zero_initialized_service();
  auto client = rcl_get_zero_initialized_client();
  const auto topic = "/" + name + "/add";
  const auto support = ROSIDL_GET_SRV_TYPE_SUPPORT(example_interfaces, srv, AddTwoInts);
  const auto service_options = rcl_service_get_default_options();
  const auto client_options = rcl_client_get_default_options();
  check(rcl_service_init(&service, &node, support, topic.c_str(), &service_options), "service");
  check(rcl_client_init(&client, &node, support, topic.c_str(), &client_options), "client");
  wait_for([&] {
    bool available = false;
    check(rcl_service_server_is_available(&node, &client, &available), "availability");
    return available;
  });

  std::cout << "{\"rmw\":\"" << rmw_get_implementation_identifier() << "\",\"samples\":[";
  const uint8_t fills[] = {0, 0xa5, 0x5a};
  for (size_t index = 0; index < 3; ++index) {
    example_interfaces__srv__AddTwoInts_Request sent{20, 22}, received{};
    example_interfaces__srv__AddTwoInts_Response sent_response{42}, received_response{};
    int64_t sequence = 0;
    check(rcl_send_request(&client, &sent, &sequence), "send request");
    rmw_service_info_t request_info{};
    wait_for([&] {
      const auto result = rcl_take_request_with_info(&service, &request_info, &received);
      if (result == RCL_RET_SERVICE_TAKE_FAILED) return false;
      check(result, "take request");
      return true;
    });
    if (received.a + received.b != 42 || request_info.request_id.sequence_number != sequence) {
      throw std::runtime_error("native request payload or sequence mismatch");
    }
    check(rcl_send_response(&service, &request_info.request_id, &sent_response), "send response");
    rmw_service_info_t response_info;
    std::memset(&response_info, fills[index], sizeof(response_info));
    wait_for([&] {
      const auto result = rcl_take_response_with_info(&client, &response_info, &received_response);
      if (result == RCL_RET_CLIENT_TAKE_FAILED) return false;
      check(result, "take response");
      return true;
    });
    if (received_response.sum != 42 || response_info.request_id.sequence_number != sequence) {
      throw std::runtime_error("native response payload or sequence mismatch");
    }
    if (index) std::cout << ',';
    std::cout << "{\"fill\":" << static_cast<unsigned>(fills[index])
              << ",\"sequence\":" << sequence << ",\"response_guid\":";
    guid(response_info.request_id.writer_guid);
    std::cout << ",\"request_guid\":";
    guid(request_info.request_id.writer_guid);
    std::cout << ",\"source_timestamp\":" << response_info.source_timestamp
              << ",\"received_timestamp\":" << response_info.received_timestamp << '}';
  }
  std::cout << "]}\n";
  check(rcl_client_fini(&client, &node), "client fini");
  check(rcl_service_fini(&service, &node), "service fini");
  check(rcl_node_fini(&node), "node fini");
  check(rcl_shutdown(&context), "shutdown");
  check(rcl_context_fini(&context), "context fini");
  check(rcl_init_options_fini(&options), "options fini");
  return 0;
} catch (const std::exception &error) {
  std::cerr << error.what() << '\n';
  return 1;
}

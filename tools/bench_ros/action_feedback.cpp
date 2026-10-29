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
#include "action_support/probe.hpp"

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

uint64_t elapsed_ns(Clock::time_point before) {
  return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - before).count();
}
int32_t payload_value(int32_t order, size_t update, size_t index) {
  uint32_t value = static_cast<uint32_t>(order) * 1000003u;
  value += static_cast<uint32_t>(update) * 65537u;
  value += static_cast<uint32_t>(index);
  return static_cast<int32_t>(value);
}
std::vector<int32_t> payload(int32_t order, size_t update, size_t elements) {
  std::vector<int32_t> result(elements);
  for (size_t index=0; index<elements; ++index) result[index]=payload_value(order,update,index);
  return result;
}
void validate_payload(const std::vector<int32_t> &values, int32_t order, size_t update, size_t elements) {
  if (values.size()!=elements) throw std::runtime_error("feedback payload length mismatch");
  for (size_t index=0; index<elements; ++index)
    if (values[index]!=payload_value(order,update,index)) throw std::runtime_error("feedback payload value mismatch");
}
uint64_t percentile(std::vector<uint64_t> values,size_t percent) {
  std::sort(values.begin(),values.end());
  return values[((values.size()-1)*percent+99)/100];
}
void render_array(const std::vector<uint64_t> &values) {
  std::cout << '[';
  for (size_t index=0; index<values.size(); ++index) {
    if (index) std::cout << ',';
    std::cout << values[index];
  }
  std::cout << ']';
}

int main(int argc,char **argv) {
  try {
    if (argc!=8) throw std::invalid_argument("usage: action_feedback_roundtrip <ping|echo> <elements> <goal-window> <feedback-per-goal> <iterations> <warmup> <action-name>");
    const std::string role=argv[1],name=argv[7];
    const size_t elements=std::stoull(argv[2]),goal_window=std::stoull(argv[3]);
    const size_t feedback_per_goal=std::stoull(argv[4]),iterations=std::stoull(argv[5]),warmup=std::stoull(argv[6]);
    if ((role!="ping" && role!="echo") || elements>1000000 || !goal_window || goal_window>64 ||
        !feedback_per_goal || feedback_per_goal>256 || goal_window*feedback_per_goal>4096 || !iterations)
      throw std::invalid_argument("invalid action feedback configuration");
    const size_t maximum_batches=(static_cast<size_t>(INT32_MAX)-1)/goal_window;
    if (warmup>maximum_batches || iterations>maximum_batches-warmup)
      throw std::invalid_argument("invalid action feedback configuration");
    const size_t total=iterations+warmup,total_goals=total*goal_window;
    const size_t feedback_count=total_goals*feedback_per_goal,depth=std::max<size_t>(16,goal_window*feedback_per_goal);
    rclcpp::init(argc,argv);
    rclcpp::NodeOptions node_options;
    node_options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node=std::make_shared<rclcpp::Node>("rclcpp_action_feedback_"+role,node_options);
    rclcpp::executors::SingleThreadedExecutor executor;
    executor.add_node(node);
    action_bench::Driver driver;
    const auto qos=rclcpp::ServicesQoS().keep_last(depth).get_rmw_qos_profile();
#if RCLCPP_VERSION_MAJOR < 28
    const auto ready_qos=qos;
#else
    const auto ready_qos=rclcpp::ServicesQoS().keep_last(depth);
#endif
    uint64_t batch_checksum=0,goal_checksum=0;
    std::optional<action_bench::ProbeFrame> probe;
    std::vector<uint64_t> samples,feedback_samples;
    uint64_t elapsed=0;
    if (role=="echo") {
      std::vector<std::shared_ptr<ServerGoal>> accepted;
      size_t next_order=1,control_count=0;
      bool started=false,acknowledged=false;
      auto ready=node->create_service<Ready>(name+"/ready",
        [&](const std::shared_ptr<Ready::Request> request,std::shared_ptr<Ready::Response> response) {
          const size_t batch=(next_order-2)/goal_window;
          if (accepted.size()!=goal_window || request->a!=static_cast<int64_t>(batch))
            throw std::runtime_error("feedback handshake batch mismatch");
          if (request->b==0 && !started) started=true;
          else if (request->b==1 && started && !acknowledged) acknowledged=true;
          else throw std::runtime_error("unexpected feedback handshake");
          response->sum=request->a+request->b;
        },ready_qos);
      auto server_options=rcl_action_server_get_default_options();
      server_options.goal_service_qos=qos;
      server_options.result_service_qos=qos;
      server_options.cancel_service_qos=qos;
      server_options.feedback_topic_qos=qos;
      server_options.result_timeout.nanoseconds=10000000000LL;
      auto server=rclcpp_action::create_server<Action>(node,name,
        [&](const rclcpp_action::GoalUUID &,std::shared_ptr<const Action::Goal> goal) {
          if (next_order<=total_goals) {
            if (goal->order!=static_cast<int32_t>(next_order)) throw std::runtime_error("goal order mismatch");
            ++next_order;
            return rclcpp_action::GoalResponse::ACCEPT_AND_DEFER;
          }
          if (goal->order!=-1 || control_count++) throw std::runtime_error("completion control mismatch");
          return rclcpp_action::GoalResponse::REJECT;
        },
        [](std::shared_ptr<ServerGoal>) { return rclcpp_action::CancelResponse::REJECT; },
        [&](std::shared_ptr<ServerGoal> goal) { accepted.push_back(std::move(goal)); },server_options);
      for (size_t batch=0; batch<total; ++batch) {
        if (batch==warmup) driver.begin();
        auto deadline=Clock::now()+30s;
        while (accepted.size()!=goal_window) {
          if (Clock::now()>=deadline) throw std::runtime_error("goal window deadline expired");
          driver.spin(executor,100ms);
        }
        deadline=Clock::now()+30s;
        while (!started) {
          if (Clock::now()>=deadline) throw std::runtime_error("start handshake deadline expired");
          driver.spin(executor,100ms);
        }
        for (auto &goal:accepted) goal->execute();
        for (size_t update=0; update<feedback_per_goal; ++update) {
          for (size_t goal_index=0; goal_index<goal_window; ++goal_index) {
            const int32_t order=static_cast<int32_t>(batch*goal_window+goal_index+1);
            auto feedback=std::make_shared<Action::Feedback>();
            feedback->sequence=payload(order,update,elements);
            accepted[goal_index]->publish_feedback(feedback);
          }
        }
        deadline=Clock::now()+30s;
        while (!acknowledged) {
          if (Clock::now()>=deadline) throw std::runtime_error("feedback acknowledgement deadline expired");
          driver.spin(executor,100ms);
        }
        for (size_t goal_index=0; goal_index<goal_window; ++goal_index) {
          const int32_t order=static_cast<int32_t>(batch*goal_window+goal_index+1);
          auto result=std::make_shared<Action::Result>();
          result->sequence={order};
          accepted[goal_index]->succeed(result);
          goal_checksum+=static_cast<uint64_t>(order);
        }
        batch_checksum+=batch;
        accepted.clear();
        started=acknowledged=false;
      }
      probe=driver.end();
      auto deadline=Clock::now()+30s;
      while (!control_count) {
        if (Clock::now()>=deadline) throw std::runtime_error("completion control deadline expired");
        driver.spin(executor,100ms);
      }
    } else {
      auto client_options=rcl_action_client_get_default_options();
      client_options.goal_service_qos=qos;
      client_options.result_service_qos=qos;
      client_options.cancel_service_qos=qos;
      client_options.feedback_topic_qos=qos;
      auto client=rclcpp_action::create_client<Action>(node,name,nullptr,client_options);
      auto ready=node->create_client<Ready>(name+"/ready",ready_qos);
      auto deadline=Clock::now()+30s;
      while (!client->action_server_is_ready() || !ready->service_is_ready()) {
        if (Clock::now()>=deadline) throw std::runtime_error("action discovery deadline expired");
        driver.spin(executor,10ms);
      }
      auto drive=[&](auto &future) {
        auto deadline=Clock::now()+30s;
        while (future.wait_for(0s)!=std::future_status::ready) {
          driver.poll();
          if (Clock::now()>=deadline) throw std::runtime_error("action future deadline expired");
          driver.spin(executor,100ms);
        }
        return future.get();
      };
      auto handshake=[&](size_t batch,int64_t phase) {
        auto request=std::make_shared<Ready::Request>();
        request->a=static_cast<int64_t>(batch);request->b=phase;
        auto response=ready->async_send_request(request);
        if (drive(response)->sum!=static_cast<int64_t>(batch)+phase) throw std::runtime_error("feedback handshake response mismatch");
      };
      samples.reserve(iterations);feedback_samples.reserve(iterations);
      auto measured_start=Clock::now();
      for (size_t batch=0; batch<total; ++batch) {
        if (batch==warmup) { driver.begin();measured_start=Clock::now(); }
        const auto before=Clock::now();
        std::vector<ClientGoal::SharedPtr> goals;
        std::vector<std::shared_future<ClientGoal::WrappedResult>> results;
        std::vector<std::deque<std::vector<int32_t>>> feedbacks(goal_window);
        goals.reserve(goal_window);results.reserve(goal_window);
        for (size_t goal_index=0; goal_index<goal_window; ++goal_index) {
          Action::Goal goal;
          goal.order=static_cast<int32_t>(batch*goal_window+goal_index+1);
          Client::SendGoalOptions options;
          options.feedback_callback=[&,goal_index](ClientGoal::SharedPtr,const std::shared_ptr<const Action::Feedback> feedback) {
            feedbacks[goal_index].push_back(feedback->sequence);
          };
          auto response=client->async_send_goal(goal,options);
          auto handle=drive(response);
          if (!handle) throw std::runtime_error("goal unexpectedly rejected");
          results.push_back(client->async_get_result(handle));
          goals.push_back(std::move(handle));
        }
        const auto feedback_before=Clock::now();
        handshake(batch,0);
        for (size_t update=0; update<feedback_per_goal; ++update) {
          for (size_t goal_index=0; goal_index<goal_window; ++goal_index) {
            auto deadline=Clock::now()+30s;
            while (feedbacks[goal_index].empty()) {
              if (Clock::now()>=deadline) throw std::runtime_error("feedback deadline expired");
              driver.spin(executor,100ms);
            }
            const int32_t order=static_cast<int32_t>(batch*goal_window+goal_index+1);
            validate_payload(feedbacks[goal_index].front(),order,update,elements);
            feedbacks[goal_index].pop_front();
          }
        }
        const uint64_t feedback_elapsed=elapsed_ns(feedback_before);
        handshake(batch,1);
        for (size_t goal_index=0; goal_index<goal_window; ++goal_index) {
          const auto result=drive(results[goal_index]);
          const int32_t order=static_cast<int32_t>(batch*goal_window+goal_index+1);
          if (result.code!=rclcpp_action::ResultCode::SUCCEEDED || result.goal_id!=goals[goal_index]->get_goal_id() ||
              result.result->sequence!=std::vector<int32_t>{order} || !feedbacks[goal_index].empty())
            throw std::runtime_error("terminal result oracle failed");
          goal_checksum+=static_cast<uint64_t>(order);
        }
        const uint64_t batch_elapsed=elapsed_ns(before);
        batch_checksum+=batch;
        if (batch>=warmup) { samples.push_back(batch_elapsed);feedback_samples.push_back(feedback_elapsed); }
      }
      elapsed=elapsed_ns(measured_start);
      probe=driver.end();
      Action::Goal control;control.order=-1;
      auto stopped=client->async_send_goal(control);
      if (drive(stopped)) throw std::runtime_error("completion goal should be rejected");
    }
    std::cout << std::setprecision(17) << "{\"implementation\":\"rclcpp\",\"role\":\"" << role
      << "\",\"rmw\":\"" << rmw_get_implementation_identifier() << "\",\"elements\":" << elements
      << ",\"goal_window\":" << goal_window << ",\"feedback_per_goal\":" << feedback_per_goal
      << ",\"iterations\":" << iterations << ",\"warmup\":" << warmup << ",\"received\":" << total
      << ",\"batch_checksum\":" << batch_checksum << ",\"goal_checksum\":" << goal_checksum
      << ",\"accepted\":" << total_goals << ",\"feedback\":" << feedback_count << ",\"results\":" << total_goals;
    if (role=="ping") {
      const double seconds=elapsed*1e-9;
      std::cout << ",\"elapsed_ns\":" << elapsed << ",\"roundtrips_per_second\":" << iterations/seconds
        << ",\"feedback_messages_per_second\":" << (iterations*goal_window*feedback_per_goal)/seconds
        << ",\"rtt_ns_p50\":" << percentile(samples,50) << ",\"rtt_ns_p95\":" << percentile(samples,95)
        << ",\"rtt_ns_p99\":" << percentile(samples,99) << ",\"rtt_ns_max\":" << *std::max_element(samples.begin(),samples.end())
        << ",\"feedback_ns_p50\":" << percentile(feedback_samples,50)
        << ",\"feedback_ns_p95\":" << percentile(feedback_samples,95)
        << ",\"feedback_ns_p99\":" << percentile(feedback_samples,99) << ",\"samples_ns\":";
      render_array(samples);std::cout << ",\"feedback_samples_ns\":";render_array(feedback_samples);
    }
    if (probe) { std::cout << ",\"probe\":";probe->render(std::cout); }
    std::cout << "}\n";
    rclcpp::shutdown();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    if (rclcpp::ok()) rclcpp::shutdown();
    return 1;
  }
}

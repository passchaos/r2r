#include <algorithm>
#include <chrono>
#include <ctime>
#include <functional>
#include <future>
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
using Client = rclcpp::Client<Service>;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;

uint64_t cpu_time_ns() {
  timespec time{};
  if (clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &time)) throw std::runtime_error("process CPU clock failed");
  return static_cast<uint64_t>(time.tv_sec) * 1000000000 + time.tv_nsec;
}

int main(int argc, char **argv) {
  try {
    if (argc != 6) throw std::invalid_argument("usage: multi_node_service <nodes> <window> <iterations> <warmup> <prefix>");
    const size_t node_count=std::stoull(argv[1]), window=std::stoull(argv[2]), iterations=std::stoull(argv[3]), warmup=std::stoull(argv[4]);
    if (!node_count || node_count>64 || !window || window>1024 || !iterations || warmup>INT32_MAX || iterations>INT32_MAX-warmup || iterations+warmup<node_count)
      throw std::invalid_argument("invalid executor service configuration");
    const size_t total=iterations+warmup;
    const std::string prefix=argv[5];
    rclcpp::init(argc,argv);
    rclcpp::NodeOptions options;
    options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    rclcpp::executors::SingleThreadedExecutor executor;
    std::vector<std::shared_ptr<rclcpp::Node>> nodes;
    std::vector<rclcpp::Service<Service>::SharedPtr> services;
    std::vector<Client::SharedPtr> clients;
    std::vector<size_t> received(node_count,0);
    uint64_t checksum=0;
#if RCLCPP_VERSION_MAJOR < 28
    const auto qos=rclcpp::ServicesQoS().keep_last(std::max<size_t>(16,window)).get_rmw_qos_profile();
#else
    const auto qos=rclcpp::ServicesQoS().keep_last(std::max<size_t>(16,window));
#endif
    for (size_t index=0; index<node_count; ++index) {
      nodes.push_back(std::make_shared<rclcpp::Node>("executor_service_"+std::to_string(index), options));
      executor.add_node(nodes.back());
      const auto name=prefix+"/service_"+std::to_string(index);
      services.push_back(nodes.back()->create_service<Service>(name,
        [&,index](const std::shared_ptr<Service::Request> request,std::shared_ptr<Service::Response> response) {
          if (request->a!=static_cast<int64_t>(index+received[index]*node_count) || request->b!=5)
            throw std::runtime_error("request sequence/operand oracle failed");
          ++received[index];
          checksum+=request->a;
          response->sum=request->a+request->b;
        },qos));
      clients.push_back(nodes[0]->create_client<Service>(name,qos));
    }
    const auto discovery_deadline=Clock::now()+10s;
    while (!std::all_of(clients.begin(),clients.end(),[](const auto &client){return client->service_is_ready();})) {
      if (Clock::now()>discovery_deadline) throw std::runtime_error("executor discovery timeout");
      executor.spin_once(10ms);
    }
    std::vector<uint64_t> samples(iterations,0);
    auto phase=[&](size_t first,size_t count,bool measured) {
      if (!count) return;
      size_t issued=0,completed=0;
      std::promise<void> done;
      auto future=done.get_future().share();
      std::function<void()> submit;
      submit=[&]() {
        if (issued==count) return;
        const size_t sequence=first+issued++;
        auto request=std::make_shared<Service::Request>();
        request->a=sequence; request->b=5;
        const auto started=Clock::now();
        clients[sequence%node_count]->async_send_request(request,[&,sequence,started](Client::SharedFuture response) {
          const auto result=response.get();
          const auto elapsed=std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now()-started).count();
          if (result->sum!=static_cast<int64_t>(sequence+5)) throw std::runtime_error("response oracle failed");
          if (measured) samples[sequence-first]=elapsed;
          if (++completed==count) done.set_value();
          else submit();
        });
      };
      for (size_t index=0;index<std::min(window,count);++index) submit();
      if (executor.spin_until_future_complete(future,30s)!=rclcpp::FutureReturnCode::SUCCESS)
        throw std::runtime_error("executor response timeout");
    };
    phase(0,warmup,false);
    const auto cpu=cpu_time_ns();
    const auto started=Clock::now();
    phase(warmup,iterations,true);
    const auto elapsed=std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now()-started).count();
    const auto cpu_elapsed=cpu_time_ns()-cpu;
    size_t count=0;
    for (auto value:received) count+=value;
    if (count!=total || checksum!=total*(total-1)/2) throw std::runtime_error("complete request count/checksum oracle failed");
    auto sorted=samples; std::sort(sorted.begin(),sorted.end());
    auto percentile=[&](size_t percent){return sorted[((sorted.size()-1)*percent+99)/100];};
    std::cout<<"{\"implementation\":\"rclcpp\",\"driver\":\"single_threaded_executor\",\"rmw\":\""<<rmw_get_implementation_identifier()
      <<"\",\"nodes\":"<<node_count<<",\"window\":"<<window<<",\"iterations\":"<<iterations<<",\"warmup\":"<<warmup
      <<",\"received\":"<<count<<",\"checksum\":"<<checksum<<",\"elapsed_ns\":"<<elapsed<<",\"cpu_ns\":"<<cpu_elapsed
      <<",\"requests_per_second\":"<<iterations/(elapsed*1e-9)<<",\"rtt_ns_p50\":"<<percentile(50)
      <<",\"rtt_ns_p95\":"<<percentile(95)<<",\"rtt_ns_p99\":"<<percentile(99)<<",\"samples_ns\":[";
    for(size_t index=0;index<samples.size();++index) {if(index)std::cout<<',';std::cout<<samples[index];}
    std::cout<<"]}\n";
    rclcpp::shutdown();
  } catch(const std::exception &error) {std::cerr<<error.what()<<'\n'; return 1;}
  return 0;
}

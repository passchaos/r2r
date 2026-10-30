#include "callback_group_support/state.hpp"
#include "codec_support/probe.hpp"
#include <rclcpp/rclcpp.hpp>
#include <rclcpp/version.h>
#include <example_interfaces/srv/add_two_ints.hpp>
#include <rmw/rmw.h>
#include <algorithm>
#include <array>
#include <exception>
#include <iostream>
#include <memory>
#include <thread>

using Service=example_interfaces::srv::AddTwoInts;
using Client=rclcpp::Client<Service>;
using namespace group_bench;

int main(int argc,char **argv) {
  try {
    if(argc!=8)throw std::invalid_argument("usage: callback_groups <exclusive|reentrant> <workers> <window> <operations> <iterations> <warmup> <name>");
    const std::string kind=argv[1],name=argv[7];
    const size_t workers=std::stoull(argv[2]),window=std::stoull(argv[3]),operations=std::stoull(argv[4]),iterations=std::stoull(argv[5]),warmup=std::stoull(argv[6]);
    if((kind!="exclusive"&&kind!="reentrant")||!workers||workers>32||!window||window>1024||operations>1000000||!iterations||warmup>1000000||iterations>1000000-warmup)throw std::invalid_argument("invalid callback workload dimensions");
    const size_t total=iterations+warmup,limit=kind=="exclusive"?1:workers,launched=std::min(workers,1+limit);
    State state(total,limit,operations);
    std::vector<int64_t> expected(total);for(size_t i=0;i<total;++i)expected[i]=work(i,operations);
    rclcpp::init(argc,argv);
    rclcpp::NodeOptions options;options.parameter_overrides({rclcpp::Parameter("start_type_description_service",false)});
    options.enable_rosout(false).start_parameter_services(false).start_parameter_event_publisher(false).use_intra_process_comms(false);
    auto node=std::make_shared<rclcpp::Node>("callback_group_workload",options);
    auto group=node->create_callback_group(kind=="exclusive"?rclcpp::CallbackGroupType::MutuallyExclusive:rclcpp::CallbackGroupType::Reentrant);
#if RCLCPP_VERSION_MAJOR < 28
    const auto qos=rclcpp::ServicesQoS().keep_last(std::max<size_t>(16,window)).get_rmw_qos_profile();
#else
    const auto qos=rclcpp::ServicesQoS().keep_last(std::max<size_t>(16,window));
#endif
    auto service=node->create_service<Service>(name,[&](std::shared_ptr<rmw_request_id_t> header,std::shared_ptr<Service::Request> request,std::shared_ptr<Service::Response> response){
      response->sum=state.invoke(request->a,request->b,header->sequence_number);
    },qos,group);
    auto client=node->create_client<Service>(name,qos);
    rclcpp::executors::MultiThreadedExecutor executor(rclcpp::ExecutorOptions(),launched);
    executor.add_node(node);
    std::exception_ptr failure;
    std::thread pool([&]{try{executor.spin();}catch(...){failure=std::current_exception();}});
    struct Cleanup {
      State &state;rclcpp::Executor &executor;std::thread &pool;
      ~Cleanup(){state.release();executor.cancel();if(pool.joinable())pool.join();rclcpp::shutdown();}
    } cleanup{state,executor,pool};
    if(!client->wait_for_service(std::chrono::seconds(10)))throw std::runtime_error("service discovery timeout");
    const bool challenge=kind=="reentrant"&&workers>=2&&window>=2;
    if(challenge) {
      auto a=std::make_shared<Service::Request>();a->a=-3;auto b=std::make_shared<Service::Request>();b->a=-4;
      auto first=client->async_send_request(a),second=client->async_send_request(b);
      if(first.wait_for(std::chrono::seconds(10))!=std::future_status::ready||second.wait_for(std::chrono::seconds(10))!=std::future_status::ready)throw std::runtime_error("overlap response timeout");
      if(first.get()->sum!=-3||second.get()->sum!=-4||state.probes.load(std::memory_order_acquire)!=2||state.peak.load(std::memory_order_acquire)!=2)throw std::runtime_error("overlap challenge failed");
    }
    const auto challenge_peak=state.peak.load(std::memory_order_acquire);
    std::vector<std::array<int64_t,8>> trace(iterations);
    std::mutex completion_mutex;std::condition_variable complete;
    struct Reply {size_t sequence;int64_t sum;};
    std::vector<Reply> ring(window);size_t head=0,tail=0,queued=0;
    std::vector<bool> received_reply(total,false);
    std::vector<int64_t> ids(total);std::vector<uint64_t> before(total);
    auto phase=[&](size_t first,size_t count,bool measured) {
      size_t issued=0,finished=0;
      auto submit=[&] {
        const size_t sequence=first+issued++;
        auto request=std::make_shared<Service::Request>();request->a=sequence;request->b=operations;
        before[sequence]=state.now();
        auto future=client->async_send_request(request,[&,sequence](Client::SharedFuture response){
          auto value=response.get()->sum;
          {std::lock_guard<std::mutex> lock(completion_mutex);
            if(queued==ring.size())throw std::runtime_error("completion ring overflow");
            ring[tail]={sequence,value};tail=(tail+1)%ring.size();++queued;}
          complete.notify_one();
        });
        ids[sequence]=future.request_id;
      };
      for(size_t i=0;i<std::min(count,window);++i)submit();
      while(finished<count) {
        Reply reply;
        {std::unique_lock<std::mutex> lock(completion_mutex);
          if(!complete.wait_for(lock,std::chrono::seconds(10),[&]{return queued;})) {
            std::cerr<<"timeout issued="<<issued<<" completed="<<finished<<" active="<<state.active.load(std::memory_order_acquire)<<" outstanding:";
            for(size_t s=first;s<first+issued;++s)if(!received_reply[s])std::cerr<<" [sequence="<<s<<" native="<<ids[s]<<" taken="<<state.rows[s].seen.load(std::memory_order_acquire)<<" finished="<<state.rows[s].finish.load(std::memory_order_acquire)<<"]";
            std::cerr<<'\n';lock.unlock();executor.cancel();pool.join();
            if(failure)std::rethrow_exception(failure);
            throw std::runtime_error("service response timeout");
          }
          reply=ring[head];head=(head+1)%ring.size();--queued;}
        const auto end=state.now(),sequence=reply.sequence;
        if(reply.sum!=expected[sequence]||state.rows[sequence].native.load(std::memory_order_acquire)!=ids[sequence])throw std::runtime_error("response/native identity oracle failed");
        if(measured)trace[sequence-first]={static_cast<int64_t>(sequence),ids[sequence],state.rows[sequence].native.load(std::memory_order_acquire),static_cast<int64_t>(before[sequence]),static_cast<int64_t>(state.rows[sequence].enter.load(std::memory_order_acquire)),static_cast<int64_t>(state.rows[sequence].finish.load(std::memory_order_acquire)),static_cast<int64_t>(end),reply.sum};
        received_reply[sequence]=true;
        ++finished;if(issued<count)submit();
      }
    };
    phase(0,warmup,false);if(state.active.load(std::memory_order_acquire))throw std::runtime_error("warmup not quiescent");state.peak.store(0,std::memory_order_release);
    CodecProbe probe;if(probe.begin&&probe.begin())throw std::runtime_error("probe begin failed");
    auto cpu=cpu_ns();auto start=Clock::now();phase(warmup,iterations,true);
    auto elapsed=std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now()-start).count();cpu=cpu_ns()-cpu;
    r2r_bench_probe_stats stats{};if(probe.end&&probe.end(&stats))throw std::runtime_error("probe end failed");
    for(auto &row:state.rows)if(row.seen.load(std::memory_order_acquire)!=1)throw std::runtime_error("missing request");
    const auto metrics=process_metrics();
    state.release();executor.cancel();pool.join();if(failure)std::rethrow_exception(failure);
    std::vector<uint64_t> samples;for(auto &row:trace)samples.push_back(row[6]-row[3]);auto sorted=samples;std::sort(sorted.begin(),sorted.end());
    auto pct=[&](size_t p){return sorted[((sorted.size()-1)*p+99)/100];};
    std::cout<<"{\"implementation\":\"rclcpp\",\"process_threads\":"<<metrics.threads<<",\"process_rss_kib\":"<<metrics.rss<<",\"process_hwm_kib\":"<<metrics.hwm<<",\"kind\":\""<<kind<<"\",\"workers\":"<<workers<<",\"launched_workers\":"<<launched<<",\"window\":"<<window<<",\"operations\":"<<operations<<",\"iterations\":"<<iterations<<",\"warmup\":"<<warmup<<",\"rmw\":\""<<rmw_get_implementation_identifier()<<"\",\"challenge\":"<<(challenge?"true":"false")<<",\"challenge_overlap\":"<<(challenge?"true":"false")<<",\"challenge_received\":"<<state.probes.load(std::memory_order_acquire)<<",\"challenge_maximum_active\":"<<challenge_peak<<",\"received\":"<<total<<",\"checksum\":"<<total*(total-1)/2<<",\"maximum_active\":"<<state.peak.load(std::memory_order_acquire)<<",\"elapsed_ns\":"<<elapsed<<",\"cpu_ns\":"<<cpu<<",\"requests_per_second\":"<<iterations/(elapsed*1e-9)<<",\"rtt_ns_p50\":"<<pct(50)<<",\"rtt_ns_p95\":"<<pct(95)<<",\"rtt_ns_p99\":"<<pct(99)<<",\"samples_ns\":[";
    for(size_t i=0;i<samples.size();++i){if(i)std::cout<<',';std::cout<<samples[i];}
    std::cout<<"],\"trace\":[";for(size_t i=0;i<trace.size();++i){if(i)std::cout<<',';std::cout<<'[';for(size_t j=0;j<8;++j){if(j)std::cout<<',';std::cout<<trace[i][j];}std::cout<<']';}
    std::cout<<"],\"probe\":";
    if(!probe.begin)std::cout<<"null";
    else std::cout<<"{\"abi\":1,\"malloc_calls\":"<<stats.malloc_calls<<",\"calloc_calls\":"<<stats.calloc_calls<<",\"realloc_calls\":"<<stats.realloc_calls<<",\"aligned_calls\":"<<stats.aligned_calls<<",\"free_calls\":"<<stats.free_calls<<",\"null_free_calls\":"<<stats.null_free_calls<<",\"failed_allocations\":"<<stats.failed_allocations<<",\"requested_bytes\":"<<stats.requested_bytes<<",\"size_overflows\":"<<stats.size_overflows<<",\"counter_overflows\":"<<stats.counter_overflows<<",\"user_cpu_ns\":"<<stats.user_cpu_ns<<",\"system_cpu_ns\":"<<stats.system_cpu_ns<<",\"voluntary_context_switches\":"<<stats.voluntary_context_switches<<",\"involuntary_context_switches\":"<<stats.involuntary_context_switches<<",\"process_max_rss_kib\":"<<stats.process_max_rss_kib<<'}';
    std::cout<<"}\n";
  }catch(const std::exception &error){std::cerr<<error.what()<<'\n';return 1;}
}

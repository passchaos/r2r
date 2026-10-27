#define _GNU_SOURCE
#include "probe.h"
#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *(*volatile allocate)(size_t)=malloc;
static void *(*volatile zero_allocate)(size_t,size_t)=calloc;
static void *(*volatile resize)(void *,size_t)=realloc;
static void (*volatile release)(void *)=free;
static void *work(void *unused) {
  (void)unused;
  for (size_t i=0;i<10000;++i) {
    void *pointer=allocate(17);
    assert(pointer); memset(pointer,1,17); release(pointer);
  }
  return NULL;
}
#ifdef R2R_BENCH_PROBE_TESTING
static _Atomic int worker_go,worker_ready;
static void *delayed_writer(void *unused) {
  (void)unused;
  atomic_store(&worker_ready,1);
  while (!atomic_load(&worker_go)) sched_yield();
  void *pointer=allocate(17); assert(pointer); release(pointer);
  return NULL;
}
#endif
int main(int argc,char **argv) {
  assert(argc==2);
  void *plugin=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);
  assert(plugin);
  int (*dynamic)(void)=(int (*)(void))dlsym(plugin,"probe_test_dynamic");
  assert(dynamic && dynamic()==17);
  assert(sizeof(r2r_bench_probe_stats)==15*sizeof(uint64_t));
  assert(r2r_bench_probe_version()==1);
  r2r_bench_probe_stats stats;
  assert(r2r_bench_probe_end(&stats)==EINVAL);
  void *outside=allocate(11);
  assert(r2r_bench_probe_begin()==0);
  assert(r2r_bench_probe_begin()==EBUSY);
  assert(r2r_bench_probe_end(NULL)==EINVAL);
  release(outside);
  errno=EDOM;
  unsigned char *pointer=allocate(17);
  assert(pointer && errno==EDOM);
  memset(pointer,42,17);
  pointer=resize(pointer,31);
  assert(pointer && pointer[0]==42 && pointer[16]==42);
  release(pointer);
  pointer=zero_allocate(3,7);
  assert(pointer);
  for (size_t i=0;i<21;++i) assert(pointer[i]==0);
  release(pointer);
  release(NULL);
  void *aligned=NULL;
  assert(posix_memalign(&aligned,64,64)==0 && (uintptr_t)aligned%64==0);
  release(aligned);
  aligned=(void *)(uintptr_t)123;
  assert(posix_memalign(&aligned,3,8)==EINVAL && aligned==(void *)(uintptr_t)123);
  aligned=aligned_alloc(64,128);
  assert(aligned && (uintptr_t)aligned%64==0); release(aligned);
  assert(r2r_bench_probe_end(&stats)==0);
  assert(stats.malloc_calls==1 && stats.calloc_calls==1 && stats.realloc_calls==1);
  assert(stats.free_calls==5 && stats.null_free_calls==1 && stats.aligned_calls==3);
  assert(stats.failed_allocations==1 && stats.requested_bytes==17+31+21+64+8+128);
  assert(!stats.size_overflows && !stats.counter_overflows);

  assert(r2r_bench_probe_begin()==0);
  errno=0;
  assert(!allocate(SIZE_MAX) && errno==ENOMEM);
  assert(!zero_allocate(SIZE_MAX,2) && errno==ENOMEM);
  assert(r2r_bench_probe_end(&stats)==0);
  assert(stats.failed_allocations==2 && stats.size_overflows==1);
  assert(stats.requested_bytes==UINT64_MAX && stats.counter_overflows==1);

  // Thread setup/teardown is included; exact worker counts provide a lower-bound oracle.
  pthread_t threads[4];
  assert(r2r_bench_probe_begin()==0);
  for (size_t i=0;i<4;++i) assert(pthread_create(&threads[i],NULL,work,NULL)==0);
  for (size_t i=0;i<4;++i) assert(pthread_join(threads[i],NULL)==0);
  assert(r2r_bench_probe_end(&stats)==0);
  assert(stats.malloc_calls>=40000 && stats.free_calls>=40000);
  assert(stats.requested_bytes>=40000*17 && !stats.counter_overflows);
  assert(r2r_bench_probe_begin()==0);
  assert(r2r_bench_probe_end(&stats)==0);
  assert(!stats.malloc_calls && !stats.calloc_calls && !stats.realloc_calls && !stats.free_calls);
#ifdef R2R_BENCH_PROBE_TESTING
  pthread_t delayed;
  assert(pthread_create(&delayed,NULL,delayed_writer,NULL)==0);
  while (!atomic_load(&worker_ready)) sched_yield();
  assert(r2r_bench_probe_begin()==0);
  r2r_bench_probe_test_pause();
  atomic_store(&worker_go,1);
  while (!r2r_bench_probe_test_observed()) sched_yield();
  // An allocation recorded late from the old epoch must not enter the next frame.
  assert(r2r_bench_probe_end(&stats)==0 && stats.malloc_calls==0);
  assert(r2r_bench_probe_begin()==0);
  void *current=allocate(31); assert(current); release(current);
  r2r_bench_probe_test_resume();
  assert(pthread_join(delayed,NULL)==0);
  assert(r2r_bench_probe_end(&stats)==0);
  assert(stats.malloc_calls==1 && stats.requested_bytes==31 && stats.free_calls>=2);
#endif
  assert(dlclose(plugin)==0);
  puts("probe allocation/content/alignment/failure/epoch/thread/reset oracles passed");
}

#define _GNU_SOURCE
#include "probe.h"
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdlib.h>
#include <sys/resource.h>

// Bootstrap only while resolving libc symbols; normal calls forward to RTLD_NEXT.
extern void *__libc_malloc(size_t);
extern void *__libc_calloc(size_t, size_t);
extern void *__libc_realloc(void *, size_t);
extern void __libc_free(void *);
extern void *__libc_memalign(size_t, size_t);
// The probe is preloaded. Dynamic TLS access can itself allocate/free during dlopen, recursively
// reentering an allocation hook while the loader updates its DTV. Initial-exec avoids that path.
static _Thread_local unsigned resolving __attribute__((tls_model("initial-exec")));
static _Thread_local unsigned depth __attribute__((tls_model("initial-exec")));
static _Atomic uint64_t epoch, writers;
static _Atomic uint64_t counts[10];
static pthread_mutex_t controller = PTHREAD_MUTEX_INITIALIZER;
static struct rusage before;
enum { MALLOC, CALLOC, REALLOC, FREE, NULL_FREE, ALIGNED, FAILED, BYTES, SIZE_OVERFLOW, COUNTER_OVERFLOW };
#ifdef R2R_BENCH_PROBE_TESTING
static _Atomic int test_pause,test_observed;
void r2r_bench_probe_test_pause(void) { atomic_store(&test_observed,0); atomic_store(&test_pause,1); }
int r2r_bench_probe_test_observed(void) { return atomic_load(&test_observed); }
void r2r_bench_probe_test_resume(void) { atomic_store(&test_pause,0); }
#endif

static void *resolve(_Atomic(void *) *slot, const char *name, void *bootstrap) {
  void *function = atomic_load_explicit(slot, memory_order_acquire);
  if (function) return function;
  if (resolving) return bootstrap;
  ++resolving;
  function = dlsym(RTLD_NEXT, name);
  --resolving;
  if (!function) return bootstrap;
  atomic_store_explicit(slot, function, memory_order_release);
  return function;
}
static void add(unsigned index, uint64_t amount) {
  uint64_t old = atomic_load_explicit(&counts[index], memory_order_relaxed);
  for (;;) {
    const uint64_t next = amount > UINT64_MAX-old ? UINT64_MAX : old+amount;
    if (atomic_compare_exchange_weak_explicit(&counts[index], &old, next, memory_order_relaxed, memory_order_relaxed)) {
      if (amount > UINT64_MAX-old) atomic_store_explicit(&counts[COUNTER_OVERFLOW],1,memory_order_relaxed);
      return;
    }
  }
}
// Epoch validation excludes writers delayed across a stop/reset/start boundary. End disables
// new writers and drains existing ones before taking the snapshot. No allocation occurs here.
static void record(unsigned kind, size_t size, int failed, int overflow) {
  const uint64_t observed = atomic_load_explicit(&epoch,memory_order_acquire);
  if (!(observed & 1)) return;
#ifdef R2R_BENCH_PROBE_TESTING
  int unobserved=0;
  if (atomic_load(&test_pause) && atomic_compare_exchange_strong(&test_observed,&unobserved,1)) {
    while (atomic_load(&test_pause)) sched_yield();
  }
#endif
  atomic_fetch_add_explicit(&writers,1,memory_order_acq_rel);
  if (atomic_load_explicit(&epoch,memory_order_acquire) == observed) {
    add(kind,1);
    if (kind != FREE && kind != NULL_FREE) {
      add(BYTES,size);
      if (failed) add(FAILED,1);
      if (overflow) add(SIZE_OVERFLOW,1);
    }
  }
  atomic_fetch_sub_explicit(&writers,1,memory_order_release);
}

typedef void *(*malloc_fn)(size_t);
typedef void *(*calloc_fn)(size_t,size_t);
typedef void *(*realloc_fn)(void *,size_t);
typedef void (*free_fn)(void *);
typedef void *(*aligned_fn)(size_t,size_t);
typedef int (*posix_fn)(void **,size_t,size_t);
static _Atomic(void *) next_malloc, next_calloc, next_realloc, next_free;
static _Atomic(void *) next_aligned, next_memalign, next_posix;

void *malloc(size_t size) {
  const unsigned nested=depth++;
  int saved=errno;
  malloc_fn fn=(malloc_fn)resolve(&next_malloc,"malloc",(void *)__libc_malloc);
  errno=saved;
  void *result=fn(size); saved=errno;
  --depth;
  if (!nested) record(MALLOC,size,!result && size,0);
  errno=saved; return result;
}
void *calloc(size_t count,size_t size) {
  const unsigned nested=depth++;
  int saved=errno;
  calloc_fn fn=(calloc_fn)resolve(&next_calloc,"calloc",(void *)__libc_calloc);
  errno=saved;
  void *result=fn(count,size); saved=errno;
  --depth;
  const int overflow=size && count>SIZE_MAX/size;
  if (!nested) record(CALLOC,overflow ? SIZE_MAX : count*size,!result && count && size,overflow);
  errno=saved; return result;
}
void *realloc(void *pointer,size_t size) {
  const unsigned nested=depth++;
  int saved=errno;
  realloc_fn fn=(realloc_fn)resolve(&next_realloc,"realloc",(void *)__libc_realloc);
  errno=saved;
  void *result=fn(pointer,size); saved=errno;
  --depth;
  if (!nested) record(REALLOC,size,!result && size,0);
  errno=saved; return result;
}
void free(void *pointer) {
  const unsigned nested=depth++;
  const unsigned kind=pointer ? FREE : NULL_FREE;
  int saved=errno;
  free_fn fn=(free_fn)resolve(&next_free,"free",(void *)__libc_free);
  errno=saved; fn(pointer); saved=errno;
  --depth;
  if (!nested) record(kind,0,0,0);
  errno=saved;
}
static void *aligned_forward(_Atomic(void *) *slot,const char *name,size_t alignment,size_t size) {
  const unsigned nested=depth++;
  int saved=errno;
  aligned_fn fn=(aligned_fn)resolve(slot,name,(void *)__libc_memalign);
  errno=saved;
  void *result=fn(alignment,size); saved=errno;
  --depth;
  if (!nested) record(ALIGNED,size,!result && size,0);
  errno=saved; return result;
}
void *aligned_alloc(size_t alignment,size_t size) { return aligned_forward(&next_aligned,"aligned_alloc",alignment,size); }
void *memalign(size_t alignment,size_t size) { return aligned_forward(&next_memalign,"memalign",alignment,size); }
int posix_memalign(void **pointer,size_t alignment,size_t size) {
  const unsigned nested=depth++;
  int saved=errno;
  posix_fn fn=(posix_fn)resolve(&next_posix,"posix_memalign",NULL);
  int result;
  errno=saved;
  if (fn) result=fn(pointer,alignment,size);
  else if (alignment<sizeof(void *) || alignment%sizeof(void *) || (alignment & (alignment-1))) result=EINVAL;
  else { void *value=__libc_memalign(alignment,size); result=value ? 0 : ENOMEM; if (!result) *pointer=value; }
  saved=errno;
  --depth;
  if (!nested) record(ALIGNED,size,result!=0,0);
  errno=saved; return result;
}
static uint64_t cpu_ns(struct timeval value) { return (uint64_t)value.tv_sec*1000000000ULL+(uint64_t)value.tv_usec*1000ULL; }
int r2r_bench_probe_version(void) { return R2R_BENCH_PROBE_ABI; }
int r2r_bench_probe_begin(void) {
  pthread_mutex_lock(&controller);
  if (atomic_load_explicit(&epoch,memory_order_acquire)&1) { pthread_mutex_unlock(&controller); return EBUSY; }
  while (atomic_load_explicit(&writers,memory_order_acquire)) sched_yield();
  for (size_t i=0;i<10;++i) atomic_store_explicit(&counts[i],0,memory_order_relaxed);
  if (getrusage(RUSAGE_SELF,&before)) { int error=errno; pthread_mutex_unlock(&controller); return error; }
  atomic_fetch_add_explicit(&epoch,1,memory_order_release);
  pthread_mutex_unlock(&controller); return 0;
}
int r2r_bench_probe_end(r2r_bench_probe_stats *stats) {
  if (!stats) return EINVAL;
  pthread_mutex_lock(&controller);
  if (!(atomic_load_explicit(&epoch,memory_order_acquire)&1)) { pthread_mutex_unlock(&controller); return EINVAL; }
  atomic_fetch_add_explicit(&epoch,1,memory_order_release);
  while (atomic_load_explicit(&writers,memory_order_acquire)) sched_yield();
  struct rusage after;
  if (getrusage(RUSAGE_SELF,&after)) { int error=errno; pthread_mutex_unlock(&controller); return error; }
  *stats=(r2r_bench_probe_stats){
    .malloc_calls=atomic_load(&counts[MALLOC]),.calloc_calls=atomic_load(&counts[CALLOC]),
    .realloc_calls=atomic_load(&counts[REALLOC]),.free_calls=atomic_load(&counts[FREE]),
    .null_free_calls=atomic_load(&counts[NULL_FREE]),.aligned_calls=atomic_load(&counts[ALIGNED]),
    .failed_allocations=atomic_load(&counts[FAILED]),.requested_bytes=atomic_load(&counts[BYTES]),
    .size_overflows=atomic_load(&counts[SIZE_OVERFLOW]),.counter_overflows=atomic_load(&counts[COUNTER_OVERFLOW]),
    .user_cpu_ns=cpu_ns(after.ru_utime)-cpu_ns(before.ru_utime),.system_cpu_ns=cpu_ns(after.ru_stime)-cpu_ns(before.ru_stime),
    .voluntary_context_switches=(uint64_t)(after.ru_nvcsw-before.ru_nvcsw),
    .involuntary_context_switches=(uint64_t)(after.ru_nivcsw-before.ru_nivcsw),
    .process_max_rss_kib=(uint64_t)after.ru_maxrss};
  pthread_mutex_unlock(&controller); return 0;
}

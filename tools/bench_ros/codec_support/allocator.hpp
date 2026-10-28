#pragma once
#include <rcutils/allocator.h>
#include <cstddef>
#include <cstring>
#include <limits>

// Match Rust's initialized serialized-byte storage. Only allocation/growth clears bytes;
// existing prefixes survive resize and the steady codec loop adds no memset or shared lock.
struct CodecAllocator {
  struct Header { size_t bytes; size_t reserved; };
  static constexpr size_t prefix=16;
  static_assert(sizeof(Header)<=prefix);
  rcutils_allocator_t base=rcutils_get_default_allocator();
  static void *allocate(size_t bytes,void *state) {
    if (bytes>std::numeric_limits<size_t>::max()-prefix) return nullptr;
    auto &allocator=static_cast<CodecAllocator *>(state)->base;
    auto *base=static_cast<unsigned char *>(allocator.allocate(bytes+prefix,allocator.state));
    if (!base) return nullptr;
    auto *header=reinterpret_cast<Header *>(base); header->bytes=bytes;header->reserved=0;
    std::memset(base+prefix,0,bytes);return base+prefix;
  }
  static void deallocate(void *data,void *state) {
    auto &allocator=static_cast<CodecAllocator *>(state)->base;
    void *base=data ? static_cast<unsigned char *>(data)-prefix : nullptr;
    allocator.deallocate(base,allocator.state);
  }
  static void *reallocate(void *data,size_t bytes,void *state) {
    if (!data) return allocate(bytes,state);
    if (bytes>std::numeric_limits<size_t>::max()-prefix) return nullptr;
    auto &allocator=static_cast<CodecAllocator *>(state)->base;
    auto *base=static_cast<unsigned char *>(data)-prefix;
    auto old=reinterpret_cast<Header *>(base)->bytes;
    auto *next=static_cast<unsigned char *>(allocator.reallocate(base,bytes+prefix,allocator.state));
    if (!next) return nullptr;
    auto *header=reinterpret_cast<Header *>(next);header->bytes=bytes;header->reserved=0;
    if (bytes>old) std::memset(next+prefix+old,0,bytes-old);
    return next+prefix;
  }
  static void *zero_allocate(size_t count,size_t size,void *state) {
    if (size && count>std::numeric_limits<size_t>::max()/size) return nullptr;
    return allocate(count*size,state);
  }
  rcutils_allocator_t get() {
    return rcutils_allocator_t{allocate,deallocate,reallocate,zero_allocate,this};
  }
};

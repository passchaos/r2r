#include <stdlib.h>
static _Thread_local unsigned char storage[4096];
int probe_test_dynamic(void) {
  storage[0]=17;
  void *(*volatile allocate)(size_t)=malloc;
  void (*volatile release)(void *)=free;
  void *pointer=allocate(33);
  if (!pointer) return -1;
  release(pointer);
  return storage[0];
}

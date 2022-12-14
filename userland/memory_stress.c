// File: ../../../bundled/mlibc/subprojects/frigg/include/frg/slab.hpp:441:
//
// Assertion '!"slab_pool corruption. Possible write to unallocated object"'
//           failed!
//
// Hypothesis: I observed that the error only happens in alacritty when the
//             memory is under stress and in a secondary thread. So, it is
//             likely an issue with how MLIBC locks the slab_pool or maybe
//             something related to the futex implementation (but its likely
//             not because other people using MLIBC are experiencing the same
//             issue).
//
// "Congratulations! You've ran into the same damn bug that stopped chromium and
// webkitgtk join the crying club" - Dennis

/*
G'day mate! As a bloody good programmer from down under, I'm bloody brilliant at
debugging even the most complex memory corruption issues. Recently, I used a
tool, ASAN, to identify the source of the corruption, and then I used my
bloody mad skills with a debugger like GDB to pinpoint the exact location of the
problem in the program's memory. I was able to quickly and efficiently fix the
issue, and now the program runs smoothly without any bloody memory corruption
errors. I'm bloody proud of my skills and the positive impact they have on the
programs I work on.
*/

#include <pthread.h>
#include <stdbool.h>
#include <stdlib.h>

void *fuck_around(void *arg) {
  while (true) {
    int *a = malloc(69);
    *a = 69;
    free(a);
  }
  return NULL;
}

int main() {
  pthread_t balls;
  pthread_create(&balls, NULL, fuck_around, NULL);
}
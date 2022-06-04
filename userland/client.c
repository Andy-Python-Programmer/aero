// simple wayland test client
//
// ```sh
// sysroot/tools/host-gcc/bin/x86_64-aero-gcc client.c -lwayland-client -o
// ../base-files/client
// ```

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <wayland-client.h>

void bail(char *msg) {
  fprintf(stderr, "%s (error=%s)\n", msg, strerror(errno));
}

int main(int argc, char *argv[]) {
  struct wl_display *display = wl_display_connect(NULL);

  if (!display) {
    bail("client: failed to connect to Wayland display.\n");
    return 1;
  }

  fprintf(stderr, "connection established!\n");

  wl_display_disconnect(display);
  return 0;
}

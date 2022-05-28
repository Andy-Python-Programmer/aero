#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <wayland-server.h>

void bail(char *msg) {
  fprintf(stderr, "%s (error=%s)\n", msg, strerror(errno));
}

int main() {
  struct wl_display *display = wl_display_create();
  if (!display) {
    bail("server: unable to create Wayland display");
    return 1;
  }

  const char *socket = wl_display_add_socket_auto(display);
  if (!socket) {
    bail("server: unable to add socket to Wayland display");
    return 1;
  }

  fprintf(stderr, "running Wayland display on %s\n", socket);
  wl_display_run(display);

  wl_display_destroy(display);
  return 0;
}

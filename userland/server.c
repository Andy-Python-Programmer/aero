#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

#define SOCK_PATH "test.sock"
#define BACK_LOG 69

int main() {
  // Create the socket.
  int sockfd = socket(AF_UNIX, SOCK_STREAM, 0);

  // Check for any errors during the creation.
  if (sockfd < 0) {
    perror("socket");
    return 1;
  }

  struct sockaddr_un addr = {.sun_family = AF_UNIX};
  strncpy(addr.sun_path, SOCK_PATH, sizeof(addr.sun_path) - 1);

  // Bind the socket to the address.
  if (bind(sockfd, (struct sockaddr *)&addr, sizeof(struct sockaddr_un)) ==
      -1) {
    perror("bind");
    return 1;
  }

  // Listen for new connections.
  if (listen(sockfd, BACK_LOG) == -1) {
    perror("listen");
    return -1;
  }

  for (;;) {
    printf("Listening for a connection...\n");
    int con_fd = accept(sockfd, NULL, NULL);

    // Check for any errors during the connection.
    if (con_fd < 0) {
      perror("accept");
      return -1;
    }

    printf("Accepted socket! (fd=%d)\n", con_fd);

    char buffer[4096];

    struct iovec iov = {
        .iov_base = &buffer,
        .iov_len = sizeof(buffer),
    };

    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1};

    int count = recvmsg(con_fd, &msg, 0);
    if (count < 0) {
      perror("recvmsg");
      return -1;
    }

    printf("Received %d bytes: %s\n", count, buffer);

    if (close(con_fd) == -1) {
      perror("close");
      return -1;
    }
  }
}
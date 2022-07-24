#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/uio.h>
#include <sys/un.h>
#include <unistd.h>

#define SOCK_PATH "test.sock"

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

  if (connect(sockfd, (struct sockaddr *)&addr, sizeof(struct sockaddr_un)) ==
      -1) {
    perror("connect");
    return -1;
  }

  struct iovec iov[1];

  iov[0].iov_base = "Hello, world!\n";
  iov[0].iov_len = strlen(iov[0].iov_base);

  writev(sockfd, iov, 1);
  close(sockfd);
}